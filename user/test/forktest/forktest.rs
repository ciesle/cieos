#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;
use core::ptr::null_mut;
use ulib::usys::*;

// 原本: xv6-riscv/user/forktest.c
#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    let _ = write(1, b"fork test\n");
    let mut n = 0;
    while n < 1000 {
        match fork() {
            Err(()) => break,
            Ok(0) => exit(0),
            Ok(_) => n += 1,
        }
    }
    if n == 1000 {
        return Err("fork claimed to work N times!".into());
    }
    for _ in 0..n {
        if wait(null_mut()).is_err() {
            return Err("wait stopped early".into());
        }
    }
    if wait(null_mut()).is_ok() {
        return Err("wait got too many".into());
    }
    let _ = write(1, b"fork test OK\n");
    Ok(())
}
