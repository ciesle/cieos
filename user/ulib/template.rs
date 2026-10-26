#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use api::file::*;
use api::utils::*;
use core::error::Error;

use ulib::file::*;
use ulib::print::*;
use ulib::usys::*;

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    todo!()
}
