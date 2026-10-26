#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;
use ulib::print::*;
mod fs;
mod memory;
mod process;
mod support;
use support::*;

// 原本: xv6-riscv/user/usertests.c。登録済みの全検査と実行順を維持します。
type Test = (unsafe fn(&str), &'static str);
const QUICKTESTS: &[Test] = &[
    (memory::copyin, "copyin"),
    (memory::copyout, "copyout"),
    (memory::copyinstr1, "copyinstr1"),
    (memory::copyinstr2, "copyinstr2"),
    (memory::copyinstr3, "copyinstr3"),
    (memory::rwsbrk, "rwsbrk"),
    (fs::truncate1, "truncate1"),
    (fs::truncate2, "truncate2"),
    (fs::truncate3, "truncate3"),
    (fs::openiputtest, "openiput"),
    (fs::exitiputtest, "exitiput"),
    (fs::iputtest, "iput"),
    (fs::opentest, "opentest"),
    (fs::writetest, "writetest"),
    (fs::writebig, "writebig"),
    (fs::createtest, "createtest"),
    (fs::dirtest, "dirtest"),
    (fs::exectest, "exectest"),
    (process::pipe1, "pipe1"),
    (process::killstatus, "killstatus"),
    (process::preempt, "preempt"),
    (process::exitwait, "exitwait"),
    (process::reparent, "reparent"),
    (process::twochildren, "twochildren"),
    (process::forkfork, "forkfork"),
    (process::forkforkfork, "forkforkfork"),
    (process::reparent2, "reparent2"),
    (process::mem, "mem"),
    (fs::sharedfd, "sharedfd"),
    (fs::fourfiles, "fourfiles"),
    (fs::createdelete, "createdelete"),
    (fs::unlinkread, "unlinkread"),
    (fs::linktest, "linktest"),
    (fs::concreate, "concreate"),
    (fs::linkunlink, "linkunlink"),
    (fs::subdir, "subdir"),
    (fs::bigwrite, "bigwrite"),
    (fs::bigfile, "bigfile"),
    (fs::fourteen, "fourteen"),
    (fs::rmdot, "rmdot"),
    (fs::dirfile, "dirfile"),
    (fs::iref, "iref"),
    (process::forktest, "forktest"),
    (memory::sbrkbasic, "sbrkbasic"),
    (memory::sbrkmuch, "sbrkmuch"),
    (memory::kernmem, "kernmem"),
    (memory::MAXVAplus, "MAXVAplus"),
    (memory::sbrkfail, "sbrkfail"),
    (memory::sbrkarg, "sbrkarg"),
    (memory::validatetest, "validatetest"),
    (memory::bsstest, "bsstest"),
    (memory::bigargtest, "bigargtest"),
    (memory::argptest, "argptest"),
    (memory::stacktest, "stacktest"),
    (memory::nowrite, "nowrite"),
    (memory::pgbug, "pgbug"),
    (memory::sbrkbugs, "sbrkbugs"),
    (memory::sbrklast, "sbrklast"),
    (memory::sbrk8000, "sbrk8000"),
    (memory::badarg, "badarg"),
    (memory::lazy_alloc, "lazy_alloc"),
    (memory::lazy_unmap, "lazy_unmap"),
    (memory::lazy_copy, "lazy_copy"),
    (memory::lazy_copyinstr, "lazy_copyinstr"),
    (memory::lazy_sbrk, "lazy_sbrk"),
    (fs::partial_write, "partial_write"),
    (fs::unlinkcwd, "unlinkcwd"),
];
const SLOWTESTS: &[Test] = &[
    (fs::bigdir, "bigdir"),
    (fs::manywrites, "manywrites"),
    (fs::badwrite, "badwrite"),
    (process::execout, "execout"),
    (fs::diskfull, "diskfull"),
    (fs::outofinodes, "outofinodes"),
];

fn run(test: &Test) -> bool {
    print!("test {}: ", test.1);
    if spawn() == 0 {
        unsafe {
            (test.0)(test.1);
        }
        exit(0);
    }
    let mut status = -1;
    let pid = wait(&mut status);
    if pid >= 0 && status == 0 {
        println!("OK");
        true
    } else {
        println!("FAILED (status {})", status);
        false
    }
}
fn runtests(tests: &[Test], just: Option<&str>, continuous: u8) -> i32 {
    let mut count = 0;
    for test in tests {
        if just.is_none_or(|n| n == test.1) {
            count += 1;
            if !run(test) && continuous != 2 {
                println!("SOME TESTS FAILED");
                return -1;
            }
        }
    }
    count
}
fn countfree() -> usize {
    let old = brk(0);
    let mut n = 0;
    while brk(PG_SIZE as isize) != usize::MAX {
        n += 1;
    }
    need(
        brk(-((brk(0) - old) as isize)) != usize::MAX,
        "countfree: cannot restore break",
    );
    n
}
#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    let (mut quick, mut continuous, mut just) = (false, 0, None);
    match argv.get(1).copied() {
        None => {}
        Some("-q") if argv.len() == 2 => quick = true,
        Some("-c") if argv.len() == 2 => continuous = 1,
        Some("-C") if argv.len() == 2 => continuous = 2,
        Some(n) if argv.len() == 2 && !n.starts_with('-') => just = Some(n),
        _ => return Err("Usage: usertests [-c] [-C] [-q] [testname]".into()),
    }
    loop {
        println!("usertests starting");
        let free0 = countfree();
        let mut count = 0;
        let n = runtests(QUICKTESTS, just, continuous);
        if n < 0 {
            if continuous != 2 {
                exit(1);
            }
        } else {
            count += n;
        }
        if !quick {
            if just.is_none() {
                println!("usertests slow tests starting");
            }
            let n = runtests(SLOWTESTS, just, continuous);
            if n < 0 {
                if continuous != 2 {
                    exit(1);
                }
            } else {
                count += n;
            }
        }
        let free1 = countfree();
        if free1 < free0 {
            println!(
                "FAILED -- lost some free pages {} (out of {})",
                free1, free0
            );
            if continuous != 2 {
                exit(1);
            }
        }
        if just.is_some() && count == 0 {
            return Err("NO TESTS EXECUTED".into());
        }
        if continuous == 0 {
            break;
        }
    }
    println!("ALL TESTS PASSED");
    Ok(())
}
