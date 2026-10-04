use crate::console::*;
use crate::syscall::*;

pub fn sys_read() -> usize {
    let p = argaddr(1);
    let n = argint(2);

    match console_read(true, p, n) {
        None => usize::MAX,
        Some(v) => v,
    }
}

pub fn sys_write() -> usize {
    let p = argaddr(1);
    let n = argint(2);

    console_write(true, p, n)
}
