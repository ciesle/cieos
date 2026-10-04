use crate::spinlock::*;
use console::*;
use core::fmt;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, Ordering};

pub static PANICKING: AtomicBool = AtomicBool::new(false); // trueならpanic messageを出力
pub static PANICKED: AtomicBool = AtomicBool::new(false); // 無限ループに入る

static PRINT_LOCK: SpinLock<u8> = SpinLock::new(0, "pr");

const DIGITS: &[u8; 16] = b"0123456789abcdef";

pub fn print_int(xx: i64, base: u64, mut sign: bool) {
    let mut buf: [u8; 20] = [1; 20];
    let mut i: usize = 0;
    if sign {
        sign = xx < 0;
    }
    let mut x: u64 = if sign { -xx as u64 } else { xx as u64 };

    loop {
        buf[i] = DIGITS[(x % base) as usize];
        i += 1;
        x /= base;
        if x == 0 {
            break;
        }
    }

    if sign {
        buf[i] = b'-';
        i += 1;
    }
    while i > 0 {
        i -= 1;
        consputc(buf[i]);
    }
}

pub fn print_ptr(mut x: usize) {
    consputc(b'0');
    consputc(b'x');
    for i in 0..(size_of::<usize>() * 2) {
        consputc(DIGITS[x >> (size_of::<usize>() * 8 - 4)]);
        x = x << 4;
    }
}

struct Console;
impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            consputc(c);
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    let mut share;
    unsafe {
        if !PANICKING.load(Ordering::Relaxed) {
            share = PRINT_LOCK.acquire();
        }
    }
    Console.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! printk {
	($($args:tt)*) => {
		$crate::printk::_print(format_args!($($args)*));
	};
}
