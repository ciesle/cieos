use super::syscall::*;
use crate::{
    memlayout::*,
    proc::{proc::*, trap::*},
};

use api::vm::*;

pub fn sys_exit() -> isize {
    let n = argraw(0) as i32;
    kexit(n);
    0
}

pub fn sys_getpid() -> isize {
    unsafe { *my_proc().pid.get() as isize }
}

pub fn sys_fork() -> isize {
    match kfork() {
        None => -1,
        Some(pid) => pid as isize,
    }
}

pub fn sys_wait() -> isize {
    match kwait(argaddr(0)) {
        None => -1,
        Some(pid) => pid as isize,
    }
}

pub const SBRK_EAGER: u32 = 1;
pub const SBRK_LAZY: u32 = 2;
/// メモリの遅延割当を行う
/// n: 割当サイズ、t: 割当モード
pub fn sys_sbrk() -> isize {
    let n = argraw(0) as isize;
    let t = argraw(1) as u32;
    let addr = my_proc().sz;
    if t == SBRK_EAGER || n < 0 {
        if !my_proc().growproc(n) {
            return -1;
        }
    } else {
        // メモリの遅延割当を行う。メモリを使った場合、vmfaultが割当を行う
        if addr + (n as usize) < addr || addr + (n as usize) > TRAPFRAME.raw() {
            return -1;
        }
        unsafe {
            (*my_proc().local.get()).sz += n as usize;
        }
    }
    addr as isize
}

pub fn sys_pause() -> isize {
    let n = argraw(0) as u64;
    let mut ticks = TICKS_LOCK.acquire();
    let ticks0 = *ticks;
    while *ticks - ticks0 < n {
        if my_proc().killed() {
            return -1;
        }
        sleep_prepare(&raw const TICKS_LOCK as *const ());
        drop(ticks);
        sleep();
        ticks = TICKS_LOCK.acquire();
    }
    0
}

/// false: 0, true: 1
pub fn sys_kill() -> isize {
    if kkill(argraw(0) as u32) { 0 } else { -1 }
}

/// clock tick interruptが何回起こったかを返す
pub fn sys_uptime() -> isize {
    let mut ticks = TICKS_LOCK.acquire();
    *ticks as isize
}
