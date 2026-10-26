//! C原本の検査条件を保つためのテスト専用呼び出しです。
#![allow(dead_code)]
use api::file::{FileOpenMode, FileState};
pub use api::param::*;
pub use ulib::usys::{exit, getpid, pause};
pub const CREATE: u32 = 0x200;
pub const RDWR: u32 = 2;
pub const WRONLY: u32 = 1;
pub const TRUNC: u32 = 0x400;
pub const BUFSZ: usize = (MAX_OPEN_BLOCKS as usize + 2) * BLOCK_SIZE as usize;
pub static mut BUF: [u8; BUFSZ] = [0; BUFSZ];
unsafe extern "C" {
    pub fn sys_read(fd: u32, buf: *mut u8, len: u32) -> isize;
    pub fn sys_write(fd: u32, buf: *const u8, len: u32) -> isize;
    pub fn sys_open(path: *const u8, mode: u32) -> isize;
    pub fn sys_unlink(path: *const u8) -> isize;
    pub fn sys_link(old: *const u8, new: *const u8) -> isize;
    pub fn sys_exec(path: *const u8, args: *const *const u8) -> isize;
    pub fn sys_wait(status: *mut i32) -> isize;
    pub fn sys_sbrk(n: isize, mode: u32) -> isize;
    pub fn sys_fstat(fd: u32, stat: *mut u8) -> isize;
}
pub fn need(ok: bool, message: &str) {
    if !ok {
        ulib::println!("{}", message);
        exit(1);
    }
}
pub fn open(path: &str, mode: u32) -> i32 {
    ulib::usys::open(path, FileOpenMode(mode))
        .map(|x| x as i32)
        .unwrap_or(-1)
}
pub fn fd(path: &str, mode: u32) -> i32 {
    let f = open(path, mode);
    need(f >= 0, "open failed");
    f
}
pub fn close(f: i32) {
    let _ = ulib::usys::close(f as u32);
}
pub fn write(f: i32, b: &[u8]) -> isize {
    unsafe { sys_write(f as u32, b.as_ptr(), b.len() as u32) }
}
pub fn read(f: i32, b: &mut [u8]) -> isize {
    unsafe { sys_read(f as u32, b.as_mut_ptr(), b.len() as u32) }
}
pub fn unlink(p: &str) -> i32 {
    ulib::usys::unlink(p).map(|_| 0).unwrap_or(-1)
}
pub fn link(a: &str, b: &str) -> i32 {
    ulib::usys::link(a, b).map(|_| 0).unwrap_or(-1)
}
pub fn mkdir(p: &str) -> i32 {
    ulib::usys::mkdir(p).map(|_| 0).unwrap_or(-1)
}
pub fn chdir(p: &str) -> i32 {
    ulib::usys::chdir(p).map(|_| 0).unwrap_or(-1)
}
pub fn dup(f: i32) -> i32 {
    ulib::usys::dup(f as u32).map(|x| x as i32).unwrap_or(-1)
}
pub fn fork() -> i32 {
    ulib::usys::fork().map(|x| x as i32).unwrap_or(-1)
}
pub fn spawn() -> i32 {
    let p = fork();
    need(p >= 0, "fork failed");
    p
}
pub fn kill(p: i32) {
    let _ = ulib::usys::kill(p as u32);
}
pub fn wait(status: &mut i32) -> i32 {
    unsafe { sys_wait(status) as i32 }
}
pub fn reap() -> i32 {
    unsafe { sys_wait(core::ptr::null_mut()) as i32 }
}
pub fn child_ok() {
    let mut st = -1;
    need(wait(&mut st) >= 0, "wait failed");
    need(st == 0, "child failed");
}
pub fn stat(f: i32) -> FileState {
    ulib::usys::fstat(f as u32).unwrap_or_else(|_| {
        need(false, "fstat failed");
        unreachable!()
    })
}
pub fn exec(path: &str, args: &[&str]) -> i32 {
    ulib::usys::exec(path, args).map(|x| x as i32).unwrap_or(-1)
}
pub fn brk(n: isize) -> usize {
    unsafe { sys_sbrk(n, 1) as usize }
}
pub fn lazy(n: isize) -> usize {
    unsafe { sys_sbrk(n, 2) as usize }
}
// 意図的な不正メモリアクセスをRustの参照やポインタ演算で表現しません。
pub unsafe fn load(addr: usize) -> u8 {
    let v: usize;
    unsafe {
        core::arch::asm!("lbu {v}, 0({p})",v=out(reg)v,p=in(reg)addr,options(nostack));
    }
    v as u8
}
pub unsafe fn store(addr: usize, value: u8) {
    unsafe {
        core::arch::asm!("sb {v}, 0({p})",v=in(reg)value as usize,p=in(reg)addr,options(nostack));
    }
}
pub fn name(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap()
}
