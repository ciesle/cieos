#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: PanicInfo) -> ! {
    loop {}
}

unsafe extern "C" {
	stack0: [byte; 4096 * NCPU];
}