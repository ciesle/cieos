#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;
use ulib::usys::*;

// 原本: xv6-riscv/user/zombie.c
#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    if matches!(fork(), Ok(pid) if pid > 0) {
        pause(5);
    }
    Ok(())
}
