core::arch::global_asm!(include_str!("usys.s"));

unsafe extern "C" {
    pub fn fork() -> usize;
    pub fn exit() -> usize;
    pub fn wait() -> usize;
    pub fn pipe() -> usize;
    pub fn read() -> usize;
    pub fn kill() -> usize;
    pub fn exec() -> usize;
    pub fn fstat() -> usize;
    pub fn chdir() -> usize;
    pub fn dup() -> usize;
    pub fn getpid() -> usize;
    pub fn sbrk() -> usize;
    pub fn pause() -> usize;
    pub fn uptime() -> usize;
    pub fn open() -> usize;
    pub fn write() -> usize;
    pub fn mknod() -> usize;
    pub fn unlink() -> usize;
    pub fn link() -> usize;
    pub fn mkdir() -> usize;
    pub fn close() -> usize;
    pub fn sync() -> usize;
}
