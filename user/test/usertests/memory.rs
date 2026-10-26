//! 原本: xv6-riscv/user/usertests.c のアドレス・メモリ検査。
#![allow(unsafe_op_in_unsafe_fn)]
use super::support::*;
use ulib::print::*;
const BAD: [usize; 5] = [
    0x80000000,
    0x3fffffe000,
    0x3ffffff000,
    0x4000000000,
    usize::MAX,
];
const MAXVA: usize = 1 << 38;
const REGION: usize = 1024 * 1024 * 1024;
static mut UNINIT: [u8; 10000] = [0; 10000];
unsafe extern "C" {
    fn sys_pipe(p: *mut u8) -> isize;
}
fn pipe() -> [i32; 2] {
    let (a, b) = ulib::usys::pipe().unwrap();
    let p = [a.fd as i32, b.fd as i32];
    core::mem::forget((a, b));
    p
}
unsafe fn word_store(p: usize, v: usize) {
    core::arch::asm!("sd {v}, 0({p})",p=in(reg)p,v=in(reg)v,options(nostack));
}
unsafe fn word_load(p: usize) -> usize {
    let v;
    core::arch::asm!("ld {v}, 0({p})",p=in(reg)p,v=out(reg)v,options(nostack));
    v
}
pub unsafe fn copyin(_: &str) {
    for a in BAD {
        let f = fd("copyin1", CREATE | WRONLY);
        need(
            sys_write(f as u32, a as *const u8, 8192) < 0,
            "write accepted invalid memory",
        );
        close(f);
        unlink("copyin1");
        need(
            sys_write(1, a as *const u8, 8192) <= 0,
            "console write accepted invalid memory",
        );
        let p = pipe();
        need(
            sys_write(p[1] as u32, a as *const u8, 8192) <= 0,
            "pipe write accepted invalid memory",
        );
        close(p[0]);
        close(p[1]);
    }
}
pub unsafe fn copyout(_: &str) {
    for a in core::iter::once(0).chain(BAD) {
        let f = fd("/bin/echo", 0);
        need(
            sys_read(f as u32, a as *mut u8, 8192) <= 0,
            "read accepted invalid memory",
        );
        close(f);
        let p = pipe();
        need(write(p[1], b"x") == 1, "pipe write failed");
        need(
            sys_read(p[0] as u32, a as *mut u8, 8192) <= 0,
            "pipe read accepted invalid memory",
        );
        close(p[0]);
        close(p[1]);
    }
}
pub unsafe fn copyinstr1(_: &str) {
    for a in BAD {
        need(
            sys_open(a as *const u8, CREATE | WRONLY) < 0,
            "open accepted invalid string",
        );
    }
}
unsafe fn bad_string(p: *const u8) {
    need(sys_unlink(p) == -1, "unlink accepted invalid string");
    need(
        sys_open(p, CREATE | WRONLY) == -1,
        "open accepted invalid string",
    );
    need(sys_link(p, p) == -1, "link accepted invalid string");
    let args = [b"xx\0".as_ptr(), core::ptr::null()];
    need(
        sys_exec(p, args.as_ptr()) == -1,
        "exec accepted invalid path",
    );
}
pub unsafe fn copyinstr2(_: &str) {
    let mut b = [b'x'; MAX_PATH + 1];
    b[MAX_PATH] = 0;
    bad_string(b.as_ptr());
    if spawn() == 0 {
        let b = &mut *(&raw mut BUF);
        b[..PG_SIZE].fill(b'x');
        b[PG_SIZE] = 0;
        let args = [b.as_ptr(), b.as_ptr(), b.as_ptr(), core::ptr::null()];
        need(
            sys_exec(b"/bin/echo\0".as_ptr(), args.as_ptr()) == -1,
            "exec accepted oversized string",
        );
        exit(747);
    }
    let mut st = 0;
    wait(&mut st);
    need(st == 747, "oversized exec child failed");
}
pub unsafe fn copyinstr3(_: &str) {
    brk(8192);
    let top = brk(0);
    if top % PG_SIZE != 0 {
        brk((PG_SIZE - top % PG_SIZE) as isize);
    }
    let top = brk(0);
    need(top % PG_SIZE == 0, "unaligned break");
    store(top - 1, b'x');
    bad_string((top - 1) as *const u8);
}
pub unsafe fn rwsbrk(_: &str) {
    let a = brk(8192);
    need(a != usize::MAX, "sbrk failed");
    need(brk(-8192) != usize::MAX, "shrink failed");
    let f = fd("rwsbrk", CREATE | WRONLY);
    need(
        sys_write(f as u32, (a + PG_SIZE) as *const u8, 1024) < 0,
        "write accepted freed memory",
    );
    close(f);
    unlink("rwsbrk");
    let f = fd("/bin/echo", 0);
    need(
        sys_read(f as u32, (a + PG_SIZE) as *mut u8, 10) < 0,
        "read accepted freed memory",
    );
    close(f);
}
pub unsafe fn sbrkbasic(_: &str) {
    if spawn() == 0 {
        let a = brk(REGION as isize);
        if a == usize::MAX {
            exit(0)
        }
        for p in (a..a + REGION).step_by(PG_SIZE) {
            store(p, 99);
        }
        exit(1);
    }
    let mut st = 0;
    wait(&mut st);
    need(st != 1, "too much memory allocated");
    let mut a = brk(0);
    for _ in 0..5000 {
        let b = brk(1);
        need(b == a, "sbrk byte increment mismatch");
        store(b, 1);
        a = b + 1;
    }
    let p = spawn();
    brk(1);
    need(brk(1) == a + 1, "post-fork sbrk mismatch");
    if p == 0 {
        exit(0)
    }
    child_ok();
}
pub unsafe fn sbrkmuch(_: &str) {
    let old = brk(0);
    let big = 100 * 1024 * 1024;
    need(brk((big - old) as isize) == old, "cannot grow to 100 MiB");
    store(big - 1, 99);
    let a = brk(0);
    need(brk(-(PG_SIZE as isize)) != usize::MAX, "shrink failed");
    need(brk(0) == a - PG_SIZE, "wrong shrunk break");
    let a = brk(0);
    need(
        brk(PG_SIZE as isize) == a && brk(0) == a + PG_SIZE,
        "regrow failed",
    );
    need(load(big - 1) != 99, "freed page not zeroed");
    let a = brk(0);
    need(brk(-((a - old) as isize)) == a, "restore break failed");
}
pub unsafe fn kernmem(_: &str) {
    for a in (0x80000000..0x80000000 + 2000000).step_by(50000) {
        if spawn() == 0 {
            let _ = load(a);
            exit(1);
        }
        let mut st = 0;
        wait(&mut st);
        need(st == -1, "kernel memory readable");
    }
}
#[allow(non_snake_case)]
pub unsafe fn MAXVAplus(_: &str) {
    let mut a = MAXVA;
    while a != 0 {
        if spawn() == 0 {
            store(a, 99);
            exit(1);
        }
        let mut st = 0;
        wait(&mut st);
        need(st == -1, "address above MAXVA writable");
        a = a.wrapping_shl(1);
    }
}
pub unsafe fn sbrkfail(_: &str) {
    let p = pipe();
    let mut pids = [-1; 10];
    let mut failed = false;
    for pid in &mut pids {
        *pid = fork();
        if *pid == 0 {
            let a = brk((100 * 1024 * 1024usize).wrapping_sub(brk(0)) as isize);
            write(p[1], if a == usize::MAX { b"0" } else { b"1" });
            loop {
                pause(1000);
            }
        }
        if *pid != -1 {
            let mut b = [0];
            read(p[0], &mut b);
            failed |= b[0] == b'0';
        }
    }
    if !failed {
        println!("no allocation failed; allocate more?");
    }
    let c = brk(PG_SIZE as isize);
    for pid in pids {
        if pid != -1 {
            kill(pid);
            reap();
        }
    }
    need(c != usize::MAX, "failed allocation leaked memory");
    close(p[0]);
    close(p[1]);
    if spawn() == 0 {
        need(
            brk((1000 * 1024 * 1024) as isize) == usize::MAX,
            "huge allocation succeeded",
        );
        exit(0);
    }
    child_ok();
}
pub unsafe fn sbrkarg(_: &str) {
    let a = brk(PG_SIZE as isize);
    let f = fd("sbrk", CREATE | WRONLY);
    unlink("sbrk");
    need(
        sys_write(f as u32, a as *const u8, PG_SIZE as u32) >= 0,
        "write allocated memory failed",
    );
    close(f);
    let a = brk(PG_SIZE as isize);
    need(
        sys_pipe(a as *mut u8) == 0,
        "pipe output to allocated memory failed",
    );
}
pub unsafe fn validatetest(_: &str) {
    for p in (0..=1100 * 1024).step_by(PG_SIZE) {
        need(
            sys_link(b"nosuchfile\0".as_ptr(), p as *const u8) == -1,
            "link accepted bad destination",
        );
    }
}
pub unsafe fn bsstest(_: &str) {
    let p = (&raw const UNINIT).cast::<u8>();
    for i in 0..10000 {
        need(p.add(i).read_volatile() == 0, "bss not zero");
    }
}
pub unsafe fn bigargtest(_: &str) {
    unlink("bigarg-ok");
    if spawn() == 0 {
        let mut big = [b' '; 400];
        big[399] = 0;
        let mut args = [big.as_ptr(); MAX_ARG];
        args[MAX_ARG - 1] = core::ptr::null();
        sys_exec(b"/bin/echo\0".as_ptr(), args.as_ptr());
        close(fd("bigarg-ok", CREATE));
        exit(0);
    }
    child_ok();
    close(fd("bigarg-ok", 0));
}
pub unsafe fn argptest(_: &str) {
    let f = fd("/bin/init", 0);
    sys_read(f as u32, brk(0).wrapping_sub(1) as *mut u8, u32::MAX);
    close(f);
}
pub unsafe fn stacktest(_: &str) {
    if spawn() == 0 {
        let sp: usize;
        core::arch::asm!("mv {}, sp",out(reg)sp,options(nostack));
        let _ = load(sp - USER_STACK * PG_SIZE);
        exit(1);
    }
    let mut st = 0;
    wait(&mut st);
    if st != -1 {
        exit(st);
    }
}
pub unsafe fn nowrite(_: &str) {
    for a in core::iter::once(0).chain(BAD) {
        if spawn() == 0 {
            core::arch::asm!("sw {v}, 0({p})",p=in(reg)a,v=in(reg)10usize,options(nostack));
            exit(0);
        }
        let mut st = 0;
        wait(&mut st);
        need(st != 0, "forbidden address writable");
    }
}
pub unsafe fn pgbug(_: &str) {
    let args = [core::ptr::null()];
    let p = 0xeaeb0b5b00002f5eusize;
    sys_exec(p as *const u8, args.as_ptr());
    sys_pipe(p as *mut u8);
}
pub unsafe fn sbrkbugs(_: &str) {
    for which in 0..3 {
        if spawn() == 0 {
            let size = brk(0);
            match which {
                0 => {
                    brk(-(size as isize));
                }
                1 => {
                    brk(3500 - size as isize);
                }
                _ => {
                    brk((10 * PG_SIZE + 2048) as isize - size as isize);
                    brk(-10);
                }
            }
            exit(0);
        }
        reap();
    }
}
pub unsafe fn sbrklast(_: &str) {
    let top = brk(0);
    if top % PG_SIZE != 0 {
        brk((PG_SIZE - top % PG_SIZE) as isize);
    }
    brk(PG_SIZE as isize);
    brk(10);
    brk(-20);
    let p = brk(0) - 64;
    store(p, b'x');
    store(p + 1, 0);
    let f = sys_open(p as *const u8, RDWR | CREATE);
    sys_write(f as u32, p as *const u8, 1);
    close(f as i32);
    let f = sys_open(p as *const u8, RDWR);
    store(p, 0);
    sys_read(f as u32, p as *mut u8, 1);
    need(load(p) == b'x', "last page copy failed");
}
pub unsafe fn sbrk8000(_: &str) {
    brk(0x80000004u32 as i32 as isize);
    let top = brk(0);
    store(top - 1, load(top - 1).wrapping_add(1));
}
pub unsafe fn badarg(_: &str) {
    let args = [0xffffffffusize as *const u8, core::ptr::null()];
    for _ in 0..50000 {
        sys_exec(b"/bin/echo\0".as_ptr(), args.as_ptr());
    }
}
pub unsafe fn lazy_alloc(_: &str) {
    let a = lazy(REGION as isize);
    need(a != usize::MAX, "lazy sbrk failed");
    for p in (a + PG_SIZE..a + REGION).step_by(64 * PG_SIZE) {
        word_store(p, p);
    }
    for p in (a + PG_SIZE..a + REGION).step_by(64 * PG_SIZE) {
        need(word_load(p) == p, "lazy memory data mismatch");
    }
}
pub unsafe fn lazy_unmap(_: &str) {
    let a = lazy(REGION as isize);
    need(a != usize::MAX, "lazy sbrk failed");
    for p in (a + PG_SIZE..a + REGION).step_by(PG_SIZE * PG_SIZE) {
        word_store(p, p);
    }
    for p in (a + PG_SIZE..a + REGION).step_by(PG_SIZE * PG_SIZE) {
        if spawn() == 0 {
            lazy(-(REGION as isize));
            word_store(p, p);
            exit(0);
        }
        let mut st = 0;
        wait(&mut st);
        need(st != 0, "lazy memory not unmapped");
    }
}
pub unsafe fn lazy_copy(_: &str) {
    let p = brk(0);
    lazy((4 * PG_SIZE) as isize);
    sys_open((p + 8192) as *const u8, 0);
    let p = brk(0);
    need(
        brk(-((p + 1) as isize)) == p,
        "sbrk underflow did not return old size",
    );
    for a in [
        0x3fffffc000usize,
        0x3fffffd000,
        0x3fffffe000,
        0x3ffffff000,
        0x4000000000,
        0x8000000000,
    ] {
        let f = fd("/bin/echo", 0);
        need(
            sys_read(f as u32, a as *mut u8, 512) < 0,
            "read accepted bad lazy address",
        );
        close(f);
        let f = fd("junk", CREATE | RDWR | TRUNC);
        need(
            sys_write(f as u32, a as *const u8, 512) < 0,
            "write accepted bad lazy address",
        );
        close(f);
    }
}
pub unsafe fn lazy_copyinstr(_: &str) {
    brk((PG_SIZE - brk(0) % PG_SIZE) as isize);
    let p = brk(0);
    need(p % PG_SIZE == 0, "unaligned break");
    lazy((2 * PG_SIZE) as isize);
    store(p + 4095, b'/');
    let f = sys_open((p + 4095) as *const u8, 0);
    need(f >= 0, "lazy string open / failed");
    need(
        stat(f as i32).inode_type == api::file::InodeType::Directory,
        "lazy string opened non-directory",
    );
    close(f as i32);
}
pub unsafe fn lazy_sbrk(_: &str) {
    let mut p = brk(0);
    while p < MAXVA - REGION {
        need(lazy(REGION as isize) != usize::MAX, "lazy grow failed");
        p = lazy(0);
    }
    let trapframe = MAXVA - 2 * PG_SIZE;
    let n = trapframe - PG_SIZE - p;
    need(lazy(n as isize) == p, "lazy grow near trapframe failed");
    let p = brk(PG_SIZE as isize);
    need(p == trapframe - PG_SIZE, "eager grow near trapframe failed");
    store(p, 1);
    need(load(p + 1) == 0, "new memory not zero");
    need(brk(1) == usize::MAX, "sbrk beyond trapframe succeeded");
    need(
        lazy(1) == usize::MAX,
        "lazy sbrk beyond trapframe succeeded",
    );
}
