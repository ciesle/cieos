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
        return Err("usage: kill pid...".into());
    }
    for arg in &argv[1..] {
        match arg
            .parse::<u32>()
            .map_err(|e| Box::new(e) as Box<dyn Error>)
            .and_then(|pid| kill(pid).map_err(Into::into))
        {
            Err(e) => fprintln!(2, "{e}"),
            _ => {}
        }
    }
    return Ok(());
}
