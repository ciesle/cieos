#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use core::error::Error;

use ulib::usys::*;

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() != 3 {
        return Err("Usage: ln old new".into());
    }
    if link(argv[1], argv[2]).is_err() {
        return Err(format!("link {} {}: failed", argv[1], argv[2]).into());
    }
    Ok(())
}
