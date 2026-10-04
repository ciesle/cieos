#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::panic::PanicInfo;

core::arch::global_asm!(include_str!("usys.s"));

unsafe extern "C" {
    fn read(fd: i32, buffer: *mut u8, length: i32) -> i32;
    fn write(fd: i32, buffer: *const u8, length: i32) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    let mut input = MaybeUninit::<[u8; 128]>::uninit();
    let buffer = input.as_mut_ptr().cast::<u8>();

    loop {
        // SAFETY: 各ポインタは指定長の読み書きが可能。
        // readが初期化した先頭nバイトだけをwriteへ渡す。
        unsafe {
            if write(1, b"read> ".as_ptr(), 6) != 6 {
                break;
            }

            let n = read(0, buffer, 128);
            if n < 0 {
                break;
            }
            if n == 0 {
                continue;
            }

            if write(1, b"echo: ".as_ptr(), 6) != 6 {
                break;
            }
            if write(1, buffer.cast_const(), n) != n {
                break;
            }
        }
    }

    unsafe {
        write(1, b"I/O failed\n".as_ptr(), 11);
    }
    loop {}
}

#[panic_handler]
fn panic_handler(_: &PanicInfo<'_>) -> ! {
    loop {}
}
