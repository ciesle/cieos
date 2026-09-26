#![no_std]
#![no_main]

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

mod memlayout;
mod param;
mod printk;
mod riscv;
mod sleeplock;
mod spinlock;
mod start;
mod uart;
use crate::printk::*;
use crate::uart::*;

#[unsafe(no_mangle)]
pub fn main() {
    easy_printf("hello world");
    printk!("Hello {}\n", "World");
    print_int(123, 10, false);
    print_ptr(main as *const () as usize);
    loop {}
}
