#![no_std]
#![no_main]

mod param;
mod riscv;
mod start;

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub fn main() {
    loop {}
}
