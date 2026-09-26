use core::fmt;
use core::mem::size_of;
use uart::consputc;

static mut PANICKING: bool = false; // trueならpanic messageを出力
static mut PANICKED: bool = false; // 無限ループに入る

static pr: SpinLock = SpinLock::new();

const DIGITS: &[u8; 16] = b"0123456789abcdef";

pub(crate) fn print_int(xx: i64, base: u64, mut sign: bool) {
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

pub(crate) fn print_ptr(mut x: usize) {
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

pub(crate) fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    unsafe {
        if !PANICKING {
            // TODO
        }
    }
    Console.write_fmt(args).unwrap();
    unsafe {
        if !PANICKING {
            // TODO
        }
    }
}

#[macro_export]
macro_rules! printk {
	($($args:tt)*) => {
		$crate::printk::_print(format_args!($($args)*));
	};
}

fn panic(s: &str) {
    unsafe {
        PANICKING = true;
    }
    printk!("panic: {}", s);
    unsafe {
        PANICKED = true;
    }
    loop {}
}

fn panic_init() {
    init_lock(&mut pr, "pr");
}
