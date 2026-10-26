#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use api::file::*;
use core::error::Error;

use ulib::usys::*;

fn cat(fd: u32) -> Result<(), Box<dyn Error>> {
    let mut buf: [u8; 512] = [0u8; 512];

    loop {
        let Ok(n) = read(fd, &mut buf) else {
            return Err("cat: read error".into());
        };
        if n == 0 {
            break;
        }
        if !matches!(write(1, &buf[..n]), Ok(m) if n == m) {
            return Err("cat: write error".into());
        }
    }
    Ok(())
}

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() < 2 {
        return cat(0);
    }
    for arg in &argv[1..] {
        let Ok(fd) = open(arg, FileOpenMode::RD_ONLY) else {
            return Err(format!("cat: cannot open {}", arg).into());
        };
        cat(fd)?;
        close(fd)?;
    }
    Ok(())
}
