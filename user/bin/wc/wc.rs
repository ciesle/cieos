#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use api::file::*;
use core::error::Error;

use ulib::print::*;
use ulib::usys::*;

fn wc(fd: u32, name: &str) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; 512];
    let (mut l, mut inword, mut w, mut c) = (0usize, false, 0usize, 0usize);
    loop {
        let Ok(n) = read(fd, &mut buf) else {
            return Err("wc: read error".into());
        };
        if n == 0 {
            break;
        }
        for &ch in &buf[..n] {
            c += 1;
            if ch == b'\n' {
                l += 1;
            }
            if b" \r\t\n\x0B".contains(&ch) {
                inword = false;
            } else if !inword {
                w += 1;
                inword = true;
            }
        }
    }
    if c > 0 {
        l += 1;
    }
    println!("{} lines, {} words, {} chars in {}", l, w, c, name);
    Ok(())
}

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() < 2 {
        return wc(0, "");
    }
    for arg in &argv[1..] {
        let Ok(fd) = open(arg, FileOpenMode::RD_ONLY) else {
            return Err(format!("wc: cannot open {}", arg).into());
        };
        wc(fd, arg)?;
        close(fd)?;
    }
    Ok(())
}
