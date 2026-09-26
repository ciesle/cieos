use crate::{memlayout::*, printk::*, riscv::*, spinlock::*};
use core::ptr::{read_volatile, write_volatile};

#[inline]
fn reg(offset: usize) -> *mut u8 {
    (UART0 + offset) as *mut u8
}

#[inline]
fn read_reg(offset: usize) -> u8 {
    unsafe { read_volatile(reg(offset)) }
}

#[inline]
fn write_reg(offset: usize, v: u8) {
    unsafe {
        write_volatile(reg(offset), v);
    }
}

const RHR: usize = 0; // recieve holding register
const THR: usize = 0; // transmit holding register
const IER: usize = 1; // interrupt enable register
const IER_RX_ENABLE: u8 = (1 << 0); // receiver interrupts
const IER_TX_ENABLE: u8 = (1 << 1); // transmit interrupts
const FCR: usize = 2; // FIFO control register
const FCR_FIFO_ENABLE: u8 = (1 << 0);
const FCR_FIFO_CLEAR: u8 = (3 << 0); // FIFOの要素をクリアする？
const ISR: usize = 2; // interrupt status register
const LCR: usize = 3; // line control register
const LCR_EIGHT_BITS: u8 = (3 << 0);
const LCR_BAUD_LATCH: u8 = (1 << 7); // baud rateを設定するための数値
const LSR: usize = 5; // line status register
const LSR_RX_READY: u8 = (1 << 0); // inputがRHRから読める
const LSR_TX_IDLE: u8 = (1 << 5); // THRは仕事がない

static tx_lock: SpinLock = SleepLock::new(); // sleep lock
static mut tx_chan: u32 = 0;

fn uart_init() {
    write_reg(IER, 0x00);

    write_reg(LCR, LCR_BAUD_LATCH); // baud rateを決めるための特別なモードに入る
    write_reg(0, 0x03);
    write_reg(1, 0x00);

    write_reg(LCR, LCR_EIGHT_BITS); // set-baud modeを抜けて、word lengthを8bitに設定

    write_reg(FCR, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);

    write_reg(IER, IER_TX_ENABLE | IER_RX_ENABLE);
}

/*
TODO
fn uart_write(buf: [char], n: usize) {
    let i: usize = 0;
    while i < n {
        if (read_reg(LSR) & LSR_TX_IDLE) != 0 {
            write_reg(THR, buf[i]);
            i += 1;
        }
    }
}
 */

pub(crate) fn uart_putc_sync(c: u8) {
    // TODO panicking

    while (read_reg(LSR) & LSR_TX_IDLE) == 0 {}
    write_reg(THR, c);
}
pub(crate) fn consputc(c: u8) {
    uart_putc_sync(c);
}
pub(crate) fn easy_printf(s: &str) {
    // TODO
    for b in s.bytes() {
        uart_putc_sync(b);
    }
}
