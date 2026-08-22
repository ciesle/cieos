#![no_std]
#![no_main]
#[panic_handler]
use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

use core::ptr::{read_volatile, write_volatile};

const UART_BASE: usize = 0x1000_0000;
const UART_RBR: *const u8 = (UART_BASE + 0x00) as *const u8; //送信
const UART_THR: *mut u8 = (UART_BASE + 0x00) as *mut u8; //受信
const UART_LSR: *const u8 = (UART_BASE + 0x05) as *const u8; //ステータス

const LSR_RX_READY: u8 = 0x01; //受信バイトあり
const LSR_TX_READY: u8 = 0x20; //送信バッファ飽きあり

fn getchar() -> u8 {
    unsafe {
        while read_volatile(UART_LSR) & LSR_RX_READY == 0 {}
        read_volatile(UART_RBR)
    }
}

fn putchar(c: char) {
    unsafe {
        while read_volatile(UART_LSR) && LSR_TX_READY == 0 {}
        write_volatile(UART_THR, c);
    }
}

fn print_string(s: &str) {
    for c in s.bytes() {
        putchar(c);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    print_string("Hello World\n");
}
