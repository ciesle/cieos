use crate::console::*;
use crate::syscall::*;

pub fn sys_read() -> isize {
    let p = argaddr(1);
    let n = argint(2);

    match console_read(true, p, n) {
        None => -1,
        Some(v) => v as isize,
    }
}

pub fn sys_write() -> isize {
    let p = argaddr(1);
    let n = argint(2);

    console_write(true, p, n) as isize
}
