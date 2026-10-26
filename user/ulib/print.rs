use crate::usys::*;
use core::fmt;

struct Console {
    fd: u32,
}
impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            match write(self.fd, &[c]) {
                Err(_) | Ok(0) => return Err(fmt::Error),
                _ => {}
            }
        }
        Ok(())
    }
}

pub fn _print(fd: u32, args: fmt::Arguments) {
    use core::fmt::Write;
    let _ = Console { fd: fd }.write_fmt(args);
}

#[macro_export]
macro_rules! print {
	($($args:tt)*) => {{
		$crate::print::_print(1, format_args!($($args)*));
	}};
}

#[macro_export]
macro_rules! println {
	($($args:tt)*) => {{
		$crate::print::_print(1, format_args!($($args)*));
		$crate::print::_print(1, format_args!("\n"));
	}};
}

#[macro_export]
macro_rules! fprint {
	($fd:expr, $($args:tt)*) => {{
		$crate::print::_print($fd, format_args!($($args)*));
	}};
}

#[macro_export]
macro_rules! fprintln {
	($fd:expr, $($args:tt)*) => {{
		let fd = $fd;
		$crate::print::_print(fd, format_args!($($args)*));
		$crate::print::_print(fd, format_args!("\n"));
	}};
}

pub use fprint;
pub use fprintln;
pub use print;
pub use println;
