use alloc::vec::*;
use core::ptr::null;

use crate::file::File;
use crate::ulib::*;
use api::{file::*, param::*};

core::arch::global_asm!(include_str!("usys.s"));

unsafe extern "C" {
    fn sys_fork() -> isize;
    fn sys_exit(code: i32) -> !;
    fn sys_wait(addr: *mut u8) -> isize; // addr: 終了コードを書き込むアドレス, ret: 子のpid
    fn sys_pipe(fdarray: *mut u8) -> isize;
    fn sys_read(fd: u32, buf: *mut u8, length: u32) -> isize;
    fn sys_kill(pid: u32) -> isize;
    fn sys_exec(path: *const u8, argv: *const u8) -> isize;
    fn sys_fstat(fd: u32, buf: *mut u8) -> isize;
    fn sys_chdir(path: *const u8) -> isize;
    fn sys_dup(fd: u32) -> isize;
    fn sys_getpid() -> isize;
    fn sys_sbrk(sz: isize, mode: u32) -> isize; // サイズ、モード
    fn sys_pause(time: u64) -> isize;
    fn sys_uptime() -> isize;
    fn sys_open(path: *const u8, omode: u32) -> isize;
    fn sys_write(fd: u32, buf: *const u8, length: u32) -> isize;
    fn sys_mknod(path: *const u8, major: u16, minor: u16) -> isize;
    fn sys_unlink(path: *const u8) -> isize;
    fn sys_link(old: *const u8, new: *const u8) -> isize;
    fn sys_mkdir(path: *const u8) -> isize;
    fn sys_close(fd: u32) -> isize;
    fn sys_sync() -> isize;
}

pub fn fork() -> Result<u32, ()> {
    match unsafe { sys_fork() } {
        -1 => Err(()),
        pid => Ok(pid as u32),
    }
}

pub fn exit(code: i32) -> ! {
    unsafe { sys_exit(code) }
}

pub fn wait(addr: *mut ()) -> Result<u32, SyscallError> {
    match unsafe { sys_wait(addr as *mut u8) } {
        -1 => Err(SyscallError("wait")),
        pid => Ok(pid as u32),
    }
}

pub fn pipe() -> Result<(File, File), IoError> {
    let mut buf = [0u32; 2];
    match unsafe { sys_pipe(buf.as_mut_ptr() as *mut u8) } {
        -1 => Err(IoError),
        _ => Ok((File::from_fd(buf[0]), File::from_fd(buf[1]))),
    }
}

pub fn read(fd: u32, buf: &mut [u8]) -> Result<usize, IoError> {
    unsafe {
        match sys_read(fd, buf.as_mut_ptr(), buf.len() as u32) {
            -1 => Err(IoError),
            len => Ok(len as usize),
        }
    }
}

pub fn kill(pid: u32) -> Result<(), SyscallError> {
    match unsafe { sys_kill(pid) } {
        -1 => Err(SyscallError("kill")),
        _ => Ok(()),
    }
}

pub fn exec(path: &str, argv: &[&str]) -> Result<u32, SyscallError> {
    if argv.len() >= MAX_ARG {
        return Err(SyscallError("exec"));
    }
    let mut patharg = [0u8; MAX_PATH];
    if path.len() >= MAX_PATH {
        return Err(SyscallError("exec"));
    }
    patharg[..path.len()].copy_from_slice(path.as_bytes());
    let mut raw: [Vec<u8>; MAX_ARG] = [const { Vec::new() }; MAX_ARG];
    let mut args: [*const u8; MAX_ARG] = [null(); MAX_ARG];
    for (i, arg) in argv.iter().enumerate() {
        raw[i].extend(arg.as_bytes());
        raw[i].push(0);
        args[i] = raw[i].as_ptr();
    }
    match unsafe { sys_exec(patharg.as_ptr(), args.as_ptr() as *const u8) } {
        -1 => Err(SyscallError("exec")),
        argc => Ok(argc as u32),
    }
}
pub fn fstat(fd: u32) -> Result<FileState, SyscallError> {
    let mut fstate = FileState::new();
    match unsafe { sys_fstat(fd, &raw mut fstate as *mut u8) } {
        -1 => Err(SyscallError("fstat")),
        _ => Ok(fstate),
    }
}

pub fn chdir(path: &str) -> Result<(), SyscallError> {
    let path = path_buffer(path)?;
    match unsafe { sys_chdir(path.as_ptr()) } {
        -1 => Err(SyscallError("chdir")),
        _ => Ok(()),
    }
}

pub fn dup(fd: u32) -> Result<u32, SyscallError> {
    match unsafe { sys_dup(fd) } {
        -1 => Err(SyscallError("dup")),
        fd => Ok(fd as u32),
    }
}

pub fn getpid() -> u32 {
    unsafe { sys_getpid() as u32 }
}

pub fn sbrk(n: isize) -> Result<*mut (), SyscallError> {
    const SBRK_EAGER: u32 = 1;
    match unsafe { sys_sbrk(n, SBRK_EAGER) } {
        -1 => Err(SyscallError("sbrk")),
        addr => Ok(addr as *mut ()),
    }
}
pub fn sbrk_lazy(n: isize) -> Result<*mut (), SyscallError> {
    const SBRK_LAZY: u32 = 2;
    match unsafe { sys_sbrk(n, SBRK_LAZY) } {
        -1 => Err(SyscallError("sbrk_lazy")),
        addr => Ok(addr as *mut ()),
    }
}

pub fn pause(time: u64) {
    unsafe { sys_pause(time) };
}

pub fn uptime() -> u64 {
    unsafe { sys_uptime() as u64 }
}

pub fn open(path: &str, omode: FileOpenMode) -> Result<u32, SyscallError> {
    let path = path_buffer(path)?;
    unsafe {
        match sys_open(path.as_ptr(), omode.bits()) {
            -1 => Err(SyscallError("open")),
            fd => Ok(fd as u32),
        }
    }
}

pub fn write(fd: u32, buf: &[u8]) -> Result<usize, IoError> {
    unsafe {
        match sys_write(fd, buf.as_ptr(), buf.len() as u32) {
            -1 => Err(IoError),
            len => Ok(len as usize),
        }
    }
}

pub fn mknod(path: &str, major: u16, minor: u16) -> Result<(), SyscallError> {
    let path = path_buffer(path)?;
    match unsafe { sys_mknod(path.as_ptr(), major, minor) } {
        -1 => Err(SyscallError("mknod")),
        _ => Ok(()),
    }
}

pub fn unlink(path: &str) -> Result<(), SyscallError> {
    let path = path_buffer(path)?;
    match unsafe { sys_unlink(path.as_ptr()) } {
        -1 => Err(SyscallError("unlink")),
        _ => Ok(()),
    }
}
pub fn link(old: &str, new: &str) -> Result<(), SyscallError> {
    let old = path_buffer(old)?;
    let new = path_buffer(new)?;
    match unsafe { sys_link(old.as_ptr(), new.as_ptr()) } {
        -1 => Err(SyscallError("link")),
        _ => Ok(()),
    }
}
pub fn mkdir(path: &str) -> Result<(), SyscallError> {
    let path = path_buffer(path)?;
    match unsafe { sys_mkdir(path.as_ptr()) } {
        -1 => Err(SyscallError("mkdir")),
        _ => Ok(()),
    }
}

pub fn close(fd: u32) -> Result<(), SyscallError> {
    match unsafe { sys_close(fd) } {
        -1 => Err(SyscallError("close")),
        _ => Ok(()),
    }
}

pub fn sync() {
    unsafe { sys_sync() };
}

fn path_buffer(path: &str) -> Result<[u8; MAX_PATH], SyscallError> {
    if path.len() >= MAX_PATH || path.as_bytes().contains(&0) {
        return Err(SyscallError("path_buffer"));
    }
    let mut buf = [0u8; MAX_PATH];
    buf[..path.len()].copy_from_slice(path.as_bytes());
    Ok(buf)
}
