#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::error::Error;

use ulib::usys::*;

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    for (i, arg) in argv[1..].iter().enumerate() {
        write(1, arg.as_bytes())?;
        write(1, if i + 1 < argv.len() - 1 { b" " } else { b"\n" })?;
    }
    Ok(())
}
