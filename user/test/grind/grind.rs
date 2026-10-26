#![no_std]
#![no_main]
extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;
#[path = "../usertests/support.rs"]
mod support;
use support::*;

// 原本: xv6-riscv/user/grind.c。乱数列と23種類の操作を維持します。
fn rand(next: &mut u64) -> u64 {
    let x = (*next % 0x7ffffffe + 1) as i64;
    let mut x = 16807 * (x % 127773) - 2836 * (x / 127773);
    if x < 0 {
        x += 0x7fffffff;
    }
    *next = (x - 1) as u64;
    *next
}
fn pipeline() {
    let (aa0, aa1) = ulib::usys::pipe().unwrap();
    let (bb0, bb1) = ulib::usys::pipe().unwrap();
    let a = [aa0.fd as i32, aa1.fd as i32];
    let b = [bb0.fd as i32, bb1.fd as i32];
    core::mem::forget((aa0, aa1, bb0, bb1));
    if spawn() == 0 {
        close(b[0]);
        close(b[1]);
        close(a[0]);
        close(1);
        need(dup(a[1]) == 1, "grind: dup failed");
        close(a[1]);
        exec("grindir/../bin/echo", &["echo", "hi"]);
        exit(2);
    }
    if spawn() == 0 {
        close(a[1]);
        close(b[0]);
        close(0);
        need(dup(a[0]) == 0, "grind: dup failed");
        close(a[0]);
        close(1);
        need(dup(b[1]) == 1, "grind: dup failed");
        close(b[1]);
        exec("/bin/cat", &["cat"]);
        exit(6);
    }
    close(a[0]);
    close(a[1]);
    close(b[1]);
    let mut buf = [0; 3];
    for i in 0..3 {
        let _ = read(b[0], &mut buf[i..i + 1]);
    }
    close(b[0]);
    child_ok();
    child_ok();
    need(&buf == b"hi\n", "grind: exec pipeline failed");
}
fn go(which: u8, mut next: u64) -> ! {
    let mut f = -1;
    let mut buf = [0u8; 999];
    let initial = brk(0);
    mkdir("grindir");
    need(chdir("grindir") == 0, "grind: chdir failed");
    chdir("/");
    let mut iters = 0u64;
    loop {
        iters = iters.wrapping_add(1);
        if iters % 500 == 0 {
            write(1, if which == 0 { b"A" } else { b"B" });
        }
        match rand(&mut next) % 23 {
            1 => close(open("grindir/../a", CREATE | RDWR)),
            2 => close(open("grindir/../grindir/../b", CREATE | RDWR)),
            3 => {
                unlink("grindir/../a");
            }
            4 => {
                need(chdir("grindir") == 0, "grind: chdir failed");
                unlink("../b");
                chdir("/");
            }
            5 => {
                close(f);
                f = open("/grindir/../a", CREATE | RDWR);
            }
            6 => {
                close(f);
                f = open("/./grindir/./../b", CREATE | RDWR);
            }
            7 => {
                write(f, &buf);
            }
            8 => {
                read(f, &mut buf);
            }
            9 => {
                mkdir("grindir/../a");
                close(open("a/../a/./a", CREATE | RDWR));
                unlink("a/a");
            }
            10 => {
                mkdir("/../b");
                close(open("grindir/../b/b", CREATE | RDWR));
                unlink("b/b");
            }
            11 => {
                unlink("b");
                link("../grindir/./../a", "../b");
            }
            12 => {
                unlink("../grindir/../a");
                link(".././b", "/grindir/../a");
            }
            13 => {
                if spawn() == 0 {
                    exit(0)
                }
                reap();
            }
            14 => {
                if spawn() == 0 {
                    fork();
                    fork();
                    exit(0)
                }
                reap();
            }
            15 => {
                brk(6011);
            }
            16 => {
                let top = brk(0);
                if top > initial {
                    brk(-((top - initial) as isize));
                }
            }
            17 => {
                let pid = spawn();
                if pid == 0 {
                    close(open("a", CREATE | RDWR));
                    exit(0)
                }
                need(chdir("../grindir/..") == 0, "grind: chdir failed");
                kill(pid);
                reap();
            }
            18 => {
                if spawn() == 0 {
                    kill(getpid() as i32);
                    exit(0)
                }
                reap();
            }
            19 => {
                let (p0, p1) = ulib::usys::pipe().unwrap();
                let p = [p0.fd as i32, p1.fd as i32];
                core::mem::forget((p0, p1));
                if spawn() == 0 {
                    fork();
                    fork();
                    need(write(p[1], b"x") == 1, "grind: pipe write failed");
                    need(read(p[0], &mut buf[..1]) == 1, "grind: pipe read failed");
                    exit(0)
                }
                close(p[0]);
                close(p[1]);
                reap();
            }
            20 => {
                if spawn() == 0 {
                    unlink("a");
                    mkdir("a");
                    chdir("a");
                    unlink("../a");
                    open("x", CREATE | RDWR);
                    unlink("x");
                    exit(0)
                }
                reap();
            }
            21 => {
                unlink("c");
                let f = fd("c", CREATE | RDWR);
                need(write(f, b"x") == 1, "grind: write failed");
                let st = stat(f);
                need(st.size == 1, "grind: wrong size");
                need(st.inum <= 200, "grind: wrong inode");
                close(f);
                unlink("c");
            }
            22 => pipeline(),
            _ => {}
        }
    }
}
#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    let mut next = 1u64;
    loop {
        let p = fork();
        if p == 0 {
            unlink("a");
            unlink("b");
            let p1 = spawn();
            if p1 == 0 {
                go(0, next ^ 31)
            }
            let p2 = spawn();
            if p2 == 0 {
                go(1, next ^ 7177)
            }
            let mut status = -1;
            wait(&mut status);
            if status != 0 {
                kill(p1);
                kill(p2);
            }
            wait(&mut status);
            exit(0);
        }
        if p > 0 {
            reap();
        }
        pause(20);
        next = next.wrapping_add(1);
    }
}
