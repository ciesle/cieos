#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
extern crate regex;
use api::file::*;
use core::error::Error;
use regex::Regex;

use ulib::usys::*;

fn grep(pattern: &Regex, fd: u32) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; 1024];

    let mut m = 0;
    loop {
        let n = read(fd, &mut buf[m..])?;
        m += n;
        let mut p: usize = 0;
        while let Some(q) = buf[p..m]
            .iter()
            .position(|&x| x == b'\n')
            .map(|x| x + p)
            .or_else(|| (n == 0).then_some(m))
        {
            let Ok(s) = core::str::from_utf8(&buf[p..q]) else {
                return Ok(());
            };
            if pattern.is_match(s) {
                write(1, &buf[p..q])?;
                write(1, b"\n");
            }
            p = q + 1;
        }
        if m > 0 {
            buf.copy_within(p..m, 0);
            m = m - p;
        }
        if n == 0 {
            break;
        }
    }
    Ok(())
}

#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    if argv.len() < 2 {
        return Err("Usage: grep pattern [file ...]".into());
    }

    let pattern = Regex::new(argv[1]).map_err(|e| format!("{e}"))?;

    if argv.len() <= 2 {
        return grep(&pattern, 0);
    }

    for arg in &argv[2..] {
        let Ok(fd) = open(arg, FileOpenMode::RD_ONLY) else {
            return Err(format!("grep: cannot open {arg}").into());
        };
        grep(&pattern, fd)?;
        close(fd)?;
    }
    Ok(())
}

// C版は*、^$とアルファベットにのみ対応しているが、
// Rustにはライブラリがあるので使う
