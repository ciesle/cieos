#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;

use ulib::print::*;
use ulib::usys::*;

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() < 2 {
        return Err("Usage: mkdir files...".into());
    }
    for arg in &argv[1..] {
        if mkdir(arg).is_err() {
            fprintln!(2, "mkdir: {} failed to create", arg);
            break;
        }
    }
    Ok(())
}
