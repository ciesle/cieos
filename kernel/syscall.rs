use crate::{proc::*, sysfile::*, vm::*};

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

// 現在のプロセスのaddrにあるu64をロードする
fn fetch_addr(addr: usize) -> Option<u64> {
    let p = my_proc();
    unsafe {
        let mut retval: u64 = 0;
        if addr >= (*p.local.get()).sz || addr + size_of::<u64>() > (*p.local.get()).sz {
            None
        } else if copyin(
            &(*p.local.get()).pagetable,
            (*p.local.get()).sz,
            &raw mut retval as usize,
            addr,
            size_of_val(&retval),
        ) {
            Some(retval)
        } else {
            None
        }
    }
}

// addrからnull-terminated stringをfetchする
fn fetch_str(addr: usize, buf: &mut [char], max: usize) -> Option<usize> {
    let p = my_proc();
    unsafe {
        copyin_str(
            &(*p.local.get()).pagetable,
            (*p.local.get()).sz,
            core::ptr::from_mut(buf) as *mut () as usize,
            addr,
            max,
        )
    }
}

fn argraw(n: usize) -> usize {
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

// 引数を数値として取得
pub fn argint(n: usize) -> usize {
    argraw(n)
}

// 引数をポインタとして取得
pub fn argaddr(n: usize) -> usize {
    argraw(n)
}

// 文字列として引数を取得する。
// 文字列帳を返す
pub fn argstr(n: usize, buf: &mut [char], max: usize) -> Option<usize> {
    let addr = argaddr(n);
    fetch_str(addr, buf, max)
}

static SYSCALLS: [fn() -> usize; 22] = [
    sys_dummy, // 1
    sys_dummy, // 2
    sys_dummy, // 3
    sys_dummy, // 4
    sys_read,  // 5
    sys_dummy, // 6
    sys_dummy, // 7
    sys_dummy, // 8
    sys_dummy, // 9
    sys_dummy, // 10
    sys_dummy, // 11
    sys_dummy, // 12
    sys_dummy, // 13
    sys_dummy, // 14
    sys_dummy, // 15
    sys_write, // 16
    sys_dummy, // 17
    sys_dummy, // 18
    sys_dummy, // 19
    sys_dummy, // 20
    sys_dummy, // 21
    sys_dummy, // 22
];

fn sys_dummy() -> usize {
    0
}

pub fn syscall() {
    let p = my_proc();
    unsafe {
        let num = (*(*p.local.get()).trapframe).a7;
        if num > 0 && num < SYSCALLS.len() {
            (*(*p.local.get()).trapframe).a0 = SYSCALLS[num - 1]();
        } else {
            printk!(
                "{} {}: unknown sys call {}\n",
                *p.pid.get(),
                (*p.local.get()).name,
                num
            );
            (*(*p.local.get()).trapframe).a0 = usize::MAX;
        }
    }
}
