#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use api::file::FileOpenMode as Mode;
use core::error::Error;
use ulib::print::*;
use ulib::usys::*;

// 原本: xv6-riscv/user/forphan.c
#[unsafe(no_mangle)]
fn main(_: &[&str]) -> Result<(), Box<dyn Error>> {
    let fd = open("file0", Mode::CREATE | Mode::WR_ONLY)?;
    let state = fstat(fd)?;
    unlink("file0")?;
    if open("file0", Mode::RD_ONLY).is_ok() {
        return Err("forphan: open succeeded after unlink".into());
    }
    println!("wait for kill and reclaim {}", state.inum);
    // 原本と同じく、開いたファイルを保持したまま強制終了を待ちます。
    loop {
        pause(1000);
    }
}
