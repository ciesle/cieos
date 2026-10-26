#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use api::file::FileOpenMode as Mode;
use core::error::Error;
use ulib::usys::*;

// 原本: xv6-riscv/user/logstress.c
// 原本の500バイトの配列へ2000バイト書く範囲外アクセスは移植しません。
const SIZE: usize = 2000;
#[unsafe(no_mangle)]
fn main(argv: &[&str]) -> Result<(), Box<dyn Error>> {
    for (i, path) in argv.iter().enumerate().skip(1) {
        match fork() {
            Err(()) => return Err("logstress: fork failed".into()),
            Ok(0) => {
                let fd = open(path, Mode::CREATE | Mode::RDWR)?;
                // スタックを消費せず、子プロセス固有の書き込み用配列を確保します。
                ulib::print::println!("0");
                let data = alloc::vec![b'0'.wrapping_add(i as u8); SIZE];
                ulib::print::println!("1");
                for _ in 0..250 {
                    ulib::print::print!("_");
                    if write(fd, &data)? != data.len() {
                        return Err("logstress: write failed".into());
                    }
                    ulib::print::print!("|");
                }
                ulib::print::println!("2");
                exit(0);
            }
            Ok(_) => {}
        }
    }
    for _ in argv.iter().skip(1) {
        let mut status = -1i32;
        wait((&raw mut status).cast())?;
        if status != 0 {
            exit(status);
        }
    }
    Ok(())
}
