#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

const UART_BASE: usize = 0x1000_0000;
const UART_RBR: *const u8 = (UART_BASE + 0x00) as *const u8;
const UART_THR: *mut u8   = (UART_BASE + 0x00) as *mut u8;
const UART_LSR: *const u8 = (UART_BASE + 0x05) as *const u8;

const LSR_RX_READY: u8 = 0x01;
const LSR_TX_READY: u8 = 0x20;

// 1バイト受信
fn getchar() -> u8 {
    unsafe {
        while read_volatile(UART_LSR) & LSR_RX_READY == 0 {}
        read_volatile(UART_RBR)
    }
}

// 1バイト送信
fn putchar(c: u8) {
    unsafe {
        while read_volatile(UART_LSR) & LSR_TX_READY == 0 {}
        write_volatile(UART_THR, c);
    }
}

fn print_string(s: &str) {
    for c in s.bytes() {
        putchar(c);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    print_string("Hello World\n");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
	loop {}
}