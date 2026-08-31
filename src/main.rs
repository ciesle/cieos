#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
	loop {}
}

fn clearbbs() {
    unsafe extern "C" {
        static mut _bss_start: u64;
        static mut _bss_end: u64;
    }

    unsafe {
        let mut p = core::ptr::addr_of_mut!(_bss_start);
        let end = core::ptr::addr_of_mut!(_bss_end);
        while p.addr() < end.addr() {
            p.write(0);
            p = p.add(1);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    clearbbs();
}