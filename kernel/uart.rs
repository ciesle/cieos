use crate::{console::*, memlayout::*, printk::*, proc::*, riscv::*, sleeplock::*, spinlock::*};
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::Ordering;

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
const FCR_FIFO_CLEAR: u8 = (3 << 1); // FIFOの要素をクリアする？
const ISR: usize = 2; // interrupt status register
const LCR: usize = 3; // line control register
const LCR_EIGHT_BITS: u8 = (3 << 0);
const LCR_BAUD_LATCH: u8 = (1 << 7); // baud rateを設定するための数値
const LSR: usize = 5; // line status register
const LSR_RX_READY: u8 = (1 << 0); // inputがRHRから読める
const LSR_TX_IDLE: u8 = (1 << 5); // THRは仕事がない

static TX_LOCK: SleepLock<u32> = SleepLock::new(0, "uart"); // sleep lock
static TX_CHAN: u32 = 0; // 送信待ちのロックチャンネル

pub fn uart_init() {
    write_reg(IER, 0x00);

    write_reg(LCR, LCR_BAUD_LATCH); // baud rateを決めるための特別なモードに入る
    write_reg(0, 0x03);
    write_reg(1, 0x00);

    write_reg(LCR, LCR_EIGHT_BITS); // set-baud modeを抜けて、word lengthを8bitに設定

    write_reg(FCR, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR); // FIFOをresetしてenable

    write_reg(IER, IER_TX_ENABLE | IER_RX_ENABLE); // transmitをenable
}

pub fn uart_write(buf: &[u8], n: usize) {
    let tx_chan = TX_LOCK.acquire();

    let mut i: usize = 0;
    while i < n {
        sleep_prepare(core::ptr::from_ref(&TX_CHAN).cast::<()>());
        if read_reg(LSR) & LSR_TX_IDLE != 0 {
            write_reg(THR, buf[i]);
            i += 1;
        } else {
            sleep();
        }
    }
}

// interruptを使わず、1バイトを書き出す。
pub fn uart_putc_sync(c: u8) {
    unsafe {
        if !PANICKING.load(Ordering::Relaxed) {
            push_off();
        }
        if PANICKED.load(Ordering::Relaxed) {
            loop {}
        }

        while (read_reg(LSR) & LSR_TX_IDLE) == 0 {}
        write_reg(THR, c);

        if !PANICKING.load(Ordering::Relaxed) {
            pop_off();
        }
    }
}

// 1バイトを読み込む
pub fn uartgetc() -> Option<u8> {
    if read_reg(LSR) & LSR_RX_READY != 0 {
        Some(read_reg(RHR))
    } else {
        None
    }
}

// uart interruptを扱う。
// 入力が来た、uartがもっと出力できるようになった
// devintr()から呼び出される
pub fn uart_intr() {
    read_reg(ISR); // 割り込みの種類を取得
    if read_reg(LSR) & LSR_TX_IDLE != 0 {
        wakeup(core::ptr::from_ref(&TX_CHAN).cast::<()>());
    }
    loop {
        let c = uartgetc();
        match c {
            Some(v) => console_intr(v),
            None => break,
        }
    }
}
