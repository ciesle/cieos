use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::print::*;
use crate::usys::*;
use api::abi::*;
use core::fmt;

unsafe extern "Rust" {
    fn main(arg: &[&str]) -> Result<(), Box<dyn core::error::Error>>;
}

#[unsafe(no_mangle)]
fn start(argc: usize, argv: *const UserArg) {
    let arg: Vec<&str> = (0..argc)
        .map(|i| unsafe {
            core::str::from_utf8(core::slice::from_raw_parts(
                (*argv.add(i)).addr as *const u8,
                (*argv.add(i)).len,
            ))
            .unwrap()
        })
        .collect();
    let r: i32 = match unsafe { main(&arg) } {
        Ok(()) => 0,
        Err(e) => {
            fprintln!(2, "{}", e);
            -1
        }
    };
    exit(r);
}

use core::panic::PanicInfo;
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    let _ = fprintln!(2, "panic: {}\n", info);
    exit(1);
}

#[derive(Debug)]
pub struct IoError;
impl core::error::Error for IoError {}
impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("file operation failed")
    }
}

#[derive(Debug)]
pub struct SyscallError(pub &'static str);
impl core::error::Error for SyscallError {}
impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "syscall {} failed", self.0)
    }
}

#[derive(Debug)]
pub struct SomeError;
impl core::error::Error for SomeError {}
impl fmt::Display for SomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("something failed")
    }
}

pub fn gets(buf: &mut [u8]) -> &[u8] {
    for (i, c) in buf.iter_mut().enumerate() {
        match read(0, core::slice::from_mut(c)) {
            Err(_) | Ok(0) => return &buf[..i],
            _ => {
                if *c == b'\n' || *c == b'\r' {
                    return &buf[..i + 1];
                }
            }
        }
    }
    buf
}
