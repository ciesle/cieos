use super::{sysfile::*, sysproc::*};
use crate::utils::printk::*;
use crate::{
    fs::log::*,
    proc::{proc::*, vm::*},
};

use api::param::*;
use api::vm::*;

const SYS_FORK: u8 = 1;
const SYS_EXIT: u8 = 2;
const SYS_WAIT: u8 = 3;
const SYS_PIPE: u8 = 4;
const SYS_READ: u8 = 5;
const SYS_KILL: u8 = 6;
const SYS_EXEC: u8 = 7;
const SYS_FSTAT: u8 = 8;
const SYS_CHDIR: u8 = 9;
const SYS_DUP: u8 = 10;
const SYS_GETPID: u8 = 11;
const SYS_SBRK: u8 = 12;
const SYS_PAUSE: u8 = 13;
const SYS_UPTIME: u8 = 14;
const SYS_OPEN: u8 = 15;
const SYS_WRITE: u8 = 16;
const SYS_MKNOD: u8 = 17;
const SYS_UNLINK: u8 = 18;
const SYS_LINK: u8 = 19;
const SYS_MKDIR: u8 = 20;
const SYS_CLOSE: u8 = 21;
const SYS_SYNC: u8 = 22;

/// 現在のプロセスのaddrにあるu64をロードする
pub fn fetch_addr(addr: VAddr) -> Option<VAddr> {
    let p = my_proc();
    unsafe {
        let mut retval: usize = 0;
        if addr.raw() >= p.sz || (addr + size_of::<usize>()).raw() > p.sz {
            None
        } else if copyin(
            &p.pagetable,
            p.sz,
            api::utils::as_bytes_mut(&mut retval),
            ARange::span(addr, size_of::<usize>()),
        ) {
            Some(VAddr(retval))
        } else {
            None
        }
    }
}

/// addrからnull-terminated stringをfetchする
pub fn fetch_str(addr: VAddr, buf: &mut [u8]) -> Option<usize> {
    let p = my_proc();
    copyin_str(
        &p.pagetable,
        p.sz,
        VAddr(buf.as_ptr() as usize),
        addr,
        buf.len(),
    )
}

pub fn argraw(n: u32) -> usize {
    let p = my_proc();
    unsafe {
        match n {
            0 => (*(*p.local.get()).trapframe).a0,
            1 => (*(*p.local.get()).trapframe).a1,
            2 => (*(*p.local.get()).trapframe).a2,
            3 => (*(*p.local.get()).trapframe).a3,
            4 => (*(*p.local.get()).trapframe).a4,
            5 => (*(*p.local.get()).trapframe).a5,
            _ => panic!("argraw"),
        }
    }
}

/// 引数をポインタとして取得
pub fn argaddr(n: u32) -> VAddr {
    VAddr(argraw(n))
}

/// 文字列として引数を取得する。
/// 文字列長を返す
pub fn argstr(n: u32, buf: &mut [u8]) -> Option<usize> {
    let addr = argaddr(n);
    fetch_str(addr, buf)
}

static SYSCALLS: [fn() -> isize; 22] = [
    sys_fork,   // 1
    sys_exit,   // 2
    sys_wait,   // 3
    sys_pipe,   // 4
    sys_read,   // 5
    sys_kill,   // 6
    sys_exec,   // 7
    sys_fstat,  // 8
    sys_chdir,  // 9
    sys_dup,    // 10
    sys_getpid, // 11
    sys_sbrk,   // 12
    sys_pause,  // 13
    sys_uptime, // 14
    sys_open,   // 15
    sys_write,  // 16
    sys_mknod,  // 17
    sys_unlink, // 18
    sys_link,   // 19
    sys_mkdir,  // 20
    sys_close,  // 21
    sys_sync,   // 22
];

pub fn syscall() {
    let p = my_proc();
    unsafe {
        let num = (*p.trapframe).a7;
        if num > 0 && num <= SYSCALLS.len() {
            (*(*p.local.get()).trapframe).a0 = SYSCALLS[num - 1]() as usize;
        } else {
            printk!("{} {}: unknown sys call {}\n", *p.pid.get(), p.name, num);
            (*(*p.local.get()).trapframe).a0 = -1isize as usize;
        }
    }
}
