#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use core::error::Error;

use ulib::usys::*;

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() < 2 {
        return Err("Usage: rm files...".into());
    }
    for arg in &argv[1..] {
        if unlink(arg).is_err() {
            return Err(format!("rm: {} failed to delete", arg).into());
        }
    }
    Ok(())
}
