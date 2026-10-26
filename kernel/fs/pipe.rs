use super::file::*;
use crate::proc::{kalloc::*, proc::*, spinlock::*, vm::*};
use core::ptr::NonNull;

use api::vm::*;

const PIPE_SIZE: usize = 512;

pub struct PipeContent {
    data: [u8; PIPE_SIZE],
    nread: usize,     // 読み込まれたバイト数
    nwrite: usize,    // 書き込まれたバイト数
    read_open: bool,  // read fdがまだ開いているか
    write_open: bool, // write fdがまだ開いているか
}

#[repr(C)]
pub struct Pipe {
    pub lk: SpinLock<PipeContent>,
}
impl Pipe {
    pub fn alloc() -> Option<(FileHandle, FileHandle)> {
        let mut f0 = FileHandle::new()?;
        let mut f1 = FileHandle::new()?;
        let pi = kalloc()?.as_ptr() as *mut Pipe;
        unsafe {
            (*pi) = Pipe {
                lk: SpinLock::new(
                    PipeContent {
                        data: [0; PIPE_SIZE],
                        read_open: true,
                        write_open: true,
                        nwrite: 0,
                        nread: 0,
                    },
                    "pipe",
                ),
            };
        }
        f0.file_type = FileType::FdPipe;
        f0.readable = true;
        f0.writable = false;
        f0.pipe = pi;
        f1.file_type = FileType::FdPipe;
        f1.readable = false;
        f1.writable = true;
        f1.pipe = pi;
        Some((f0, f1))
    }
    pub fn close(&self, writable: bool) {
        let mut guard = self.lk.acquire();
        if writable {
            guard.write_open = false;
            wakeup(core::ptr::from_mut(&mut guard.nread).cast());
        } else {
            guard.read_open = false;
            wakeup(core::ptr::from_mut(&mut guard.nwrite).cast());
        }
        if !guard.read_open && !guard.write_open {
            drop(guard);
            kfree(Frame(PAddr(core::ptr::from_ref(self) as usize)));
        }
    }
    pub fn write(&self, addr: ARange<VAddr>) -> Option<u32> {
        let mut guard = self.lk.acquire();
        let p = my_proc();
        let mut i = 0;
        while i < addr.len() {
            // readが閉じていると何もしない
            if !guard.read_open || p.killed() {
                wakeup(core::ptr::from_ref(&guard.nread).cast());
                return None;
            }
            // パイプがいっぱいの場合
            if guard.nwrite == guard.nread + PIPE_SIZE {
                wakeup(core::ptr::from_ref(&guard.nread).cast());
                sleep_prepare(core::ptr::from_ref(&guard.nwrite).cast());
                drop(guard);
                sleep();
                guard = self.lk.acquire();
            } else {
                let mut ch: u8 = 0;
                if !copyin(
                    &p.pagetable,
                    p.sz,
                    unsafe { core::slice::from_raw_parts_mut(&raw mut ch, 1) },
                    ARange::span(addr.start + i as usize, 1),
                ) {
                    wakeup(core::ptr::from_ref(&guard.nread).cast());
                    return if i == 0 { None } else { Some(i as u32) };
                }
                let nwrite = guard.nwrite;
                guard.data[(nwrite % PIPE_SIZE) as usize] = ch;
                guard.nwrite += 1;
                i += 1;
            }
        }
        wakeup(core::ptr::from_ref(&guard.nread).cast());
        Some(i as u32)
    }
    pub fn read(&self, addr: ARange<VAddr>) -> Option<u32> {
        let mut guard = self.lk.acquire();
        let p = my_proc();
        let mut i = 0;

        // パイプが空
        while guard.nread == guard.nwrite && guard.write_open {
            if p.killed() {
                return None;
            }
            sleep_prepare(core::ptr::from_ref(&guard.nread).cast());
            drop(guard);
            sleep();
            guard = self.lk.acquire();
        }
        while i < addr.len() {
            if guard.nread == guard.nwrite {
                break;
            }
            let ch = guard.data[(guard.nread % PIPE_SIZE) as usize];
            unsafe {
                if !copyout(
                    &p.pagetable,
                    p.sz,
                    ARange::span(addr.start + i as usize, 1),
                    &mut ch.to_ne_bytes(),
                ) {
                    wakeup(core::ptr::from_ref(&guard.nwrite).cast());
                    return if i == 0 { None } else { Some(i as u32) };
                }
                guard.nread += 1;
                i += 1;
            }
        }
        wakeup(core::ptr::from_ref(&guard.nwrite).cast());
        Some(i as u32)
    }
}
