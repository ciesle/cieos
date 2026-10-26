//! 原本: xv6-riscv/user/usertests.c のプロセス・パイプ検査。
#![allow(unsafe_op_in_unsafe_fn)]
use super::support::*;
use ulib::print::*;
fn pipe() -> [i32; 2] {
    let (a, b) = ulib::usys::pipe().unwrap();
    let p = [a.fd as i32, b.fd as i32];
    core::mem::forget((a, b));
    p
}
pub unsafe fn pipe1(_: &str) {
    let p = pipe();
    let pid = spawn();
    let b = &mut *(&raw mut BUF);
    let mut seq = 0u32;
    if pid == 0 {
        close(p[0]);
        for _ in 0..5 {
            for x in &mut b[..1033] {
                *x = seq as u8;
                seq = seq.wrapping_add(1);
            }
            need(write(p[1], &b[..1033]) == 1033, "pipe write failed");
        }
        exit(0);
    }
    close(p[1]);
    let mut total = 0;
    let mut count = 1;
    loop {
        let n = read(p[0], &mut b[..count]);
        if n <= 0 {
            break;
        }
        for &x in &b[..n as usize] {
            need(x == seq as u8, "pipe data mismatch");
            seq = seq.wrapping_add(1);
        }
        total += n;
        count = (count * 2).min(BUFSZ);
    }
    need(total == 5 * 1033, "pipe total mismatch");
    close(p[0]);
    child_ok();
}
pub unsafe fn killstatus(_: &str) {
    for _ in 0..100 {
        let p = spawn();
        if p == 0 {
            loop {
                getpid();
            }
        }
        pause(1);
        kill(p);
        let mut st = 0;
        need(wait(&mut st) == p && st == -1, "killed child status != -1");
    }
}
pub unsafe fn preempt(_: &str) {
    let a = spawn();
    if a == 0 {
        loop {
            core::hint::spin_loop();
        }
    }
    let b = spawn();
    if b == 0 {
        loop {
            core::hint::spin_loop();
        }
    }
    let p = pipe();
    let c = spawn();
    if c == 0 {
        close(p[0]);
        need(write(p[1], b"x") == 1, "preempt write failed");
        close(p[1]);
        loop {
            core::hint::spin_loop();
        }
    }
    close(p[1]);
    need(read(p[0], &mut *(&raw mut BUF)) == 1, "preempt read failed");
    close(p[0]);
    print!("kill... ");
    kill(a);
    kill(b);
    kill(c);
    print!("wait... ");
    reap();
    reap();
    reap();
}
pub unsafe fn exitwait(_: &str) {
    for i in 0..100 {
        let p = spawn();
        if p == 0 {
            exit(i);
        }
        let mut st = -1;
        need(wait(&mut st) == p, "wait wrong pid");
        need(st == i, "wait wrong exit status");
    }
}
pub unsafe fn reparent(_: &str) {
    let master = getpid();
    for _ in 0..200 {
        let p = spawn();
        if p != 0 {
            need(reap() == p, "wait wrong pid");
        } else {
            if fork() < 0 {
                kill(master as i32);
                exit(1);
            }
            exit(0);
        }
    }
}
pub unsafe fn twochildren(_: &str) {
    for _ in 0..1000 {
        if spawn() == 0 {
            exit(0);
        }
        if spawn() == 0 {
            exit(0);
        }
        reap();
        reap();
    }
}
pub unsafe fn forkfork(_: &str) {
    for _ in 0..2 {
        if spawn() == 0 {
            for _ in 0..200 {
                if spawn() == 0 {
                    exit(0)
                }
                reap();
            }
            exit(0);
        }
    }
    child_ok();
    child_ok();
}
pub unsafe fn forkforkfork(_: &str) {
    unlink("stopforking");
    if spawn() == 0 {
        loop {
            if open("stopforking", 0) >= 0 {
                exit(0)
            }
            if fork() < 0 {
                close(open("stopforking", CREATE | RDWR));
            }
        }
    }
    pause(20);
    close(open("stopforking", CREATE | RDWR));
    reap();
    pause(10);
}
pub unsafe fn reparent2(_: &str) {
    for _ in 0..800 {
        if spawn() == 0 {
            fork();
            fork();
            exit(0)
        }
        reap();
    }
}
pub unsafe fn mem(_: &str) {
    if spawn() == 0 {
        let layout = core::alloc::Layout::from_size_align(10001, 8).unwrap();
        let mut head = core::ptr::null_mut::<u8>();
        loop {
            let p = alloc::alloc::alloc(layout);
            if p.is_null() {
                break;
            }
            p.cast::<*mut u8>().write(head);
            head = p;
        }
        while !head.is_null() {
            let next = head.cast::<*mut u8>().read();
            alloc::alloc::dealloc(head, layout);
            head = next;
        }
        let layout = core::alloc::Layout::from_size_align(20 * 1024, 8).unwrap();
        let p = alloc::alloc::alloc(layout);
        need(!p.is_null(), "cannot allocate after freeing memory");
        alloc::alloc::dealloc(p, layout);
        exit(0);
    }
    let mut st = -1;
    wait(&mut st);
    if st != -1 {
        exit(st);
    }
}
pub unsafe fn forktest(_: &str) {
    let mut n = 0;
    while n < 1000 {
        let p = fork();
        if p < 0 {
            break;
        }
        if p == 0 {
            exit(0)
        }
        n += 1;
    }
    need(n > 0, "no fork at all");
    need(n < 1000, "fork succeeded 1000 times");
    for _ in 0..n {
        need(reap() >= 0, "wait stopped early");
    }
    need(reap() == -1, "wait got too many children");
}
pub unsafe fn execout(_: &str) {
    for avail in 0..15 {
        if spawn() == 0 {
            loop {
                let a = brk(PG_SIZE as isize);
                if a == usize::MAX {
                    break;
                }
                store(a + PG_SIZE - 1, 1);
            }
            for _ in 0..avail {
                brk(-(PG_SIZE as isize));
            }
            close(1);
            let args = [b"echo\0".as_ptr(), b"x\0".as_ptr(), core::ptr::null()];
            sys_exec(b"/bin/echo\0".as_ptr(), args.as_ptr());
            exit(0);
        }
        reap();
    }
}
