#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use api::file::FileOpenMode as Mode;
use core::error::Error;
use core::ptr::null_mut;
use ulib::print::*;
use ulib::usys::*;

// 原本: xv6-riscv/user/stressfs.c
#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    println!("stressfs starting");
    let mut data = [b'a'; 512];
    let mut i = 0;
    while i < 4 {
        if matches!(fork(), Ok(pid) if pid > 0) {
            break;
        }
        i += 1;
    }
    println!("write {}", i);
    let mut path = *b"stressfs0";
    path[8] += i;
    let path = core::str::from_utf8(&path)?;
    let fd = open(path, Mode::CREATE | Mode::RDWR)?;
    for _ in 0..20 {
        let _ = write(fd, &data);
    }
    close(fd)?;
    println!("read");
    let fd = open(path, Mode::RD_ONLY)?;
    for _ in 0..20 {
        let _ = read(fd, &mut data);
    }
    close(fd)?;
    let _ = wait(null_mut());
    Ok(())
}
