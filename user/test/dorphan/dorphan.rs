#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;
use ulib::print::*;
use ulib::usys::*;

// 原本: xv6-riscv/user/dorphan.c
#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    mkdir("dd")?;
    chdir("dd")?;
    unlink("../dd")?;
    println!("wait for kill and reclaim");
    loop {
        pause(1000);
    }
}
