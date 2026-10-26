use core::cell::OnceCell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::ptr::null;
use core::ptr::null_mut;

use super::{inode::*, log::*, pipe::*};
use crate::proc::{proc::*, spinlock::*};

use api::file::*;
use api::param::*;
use api::vm::*;

#[derive(PartialEq, Eq)]
#[repr(u16)]
pub enum FileType {
    FdNone,
    FdPipe,
    FdInode,
    FdDevice,
}

pub struct File {
    pub file_type: FileType,
    pub ref_cnt: usize,
    pub readable: bool,
    pub writable: bool,
    pub pipe: *mut Pipe,
    pub ip: InodeHandle,
    pub off: u32,   // FD_INODE
    pub major: u16, // FD_DEVICE
}
impl File {
    pub const fn new() -> Self {
        Self {
            file_type: FileType::FdNone,
            ref_cnt: 0,
            readable: false,
            writable: false,
            pipe: null_mut(),
            ip: InodeHandle::new(),
            off: 0,
            major: 0,
        }
    }

    /// selfからaddrに読み込む
    /// 読み込みを行うとき、addrからnの範囲は割当済みだと保証する
    pub fn read(&mut self, addr: ARange<VAddr>) -> Option<usize> {
        if !self.readable {
            return None;
        }
        match self.file_type {
            FileType::FdPipe => unsafe { (*self.pipe).read(addr) },
            FileType::FdDevice => DEV_SW
                .get(self.major as usize)
                .and_then(|dev| dev.read(true, addr)),
            FileType::FdInode => unsafe {
                let mut guard = self.ip.lock();
                let ret = guard
                    .read(true, addr, self.off, addr.len() as u32)
                    .inspect(|sz| self.off += sz);
                self.ip.unlock();
                ret
            },
            _ => panic!("fileread"),
        }
        .map(|x| x as usize)
    }
    /// addrからselfに書き込む
    pub fn write(&mut self, addr: ARange<VAddr>) -> Option<usize> {
        if !self.writable {
            return None;
        }
        match self.file_type {
            FileType::FdPipe => unsafe { (*self.pipe).write(addr) },
            FileType::FdDevice => DEV_SW
                .get(self.major as usize)
                .and_then(|dev| dev.write(true, addr)),
            FileType::FdInode => {
                // ログトランザクションサイズを超えるのを避けるため、
                // 一度に数ブロックずつ書き込む
                // -1: inodeブロック
                // -1: 間接ブロック（addr[NDIRECT]）
                // -2: 境界に揃っていない書き込み
                // /2: bitmapとデータブロック
                let max = (MAX_OPEN_BLOCKS - 1 - 1 - 2) / 2 * BLOCK_SIZE;
                let mut i: usize = 0;
                while i < addr.len() {
                    unsafe {
                        let n1 = core::cmp::min((addr.len() - i) as u32, max);
                        let handle = LogHandle::new();
                        let mut guard = self.ip.lock();

                        let res = guard
                            .write(
                                true,
                                ARange::span(addr.start + i, n1 as usize),
                                self.off,
                                n1,
                                &handle,
                            )
                            .inspect(|r| self.off += r);
                        self.ip.unlock();
                        if let Some(r) = res
                            && r == n1
                        {
                            i += r as usize;
                        } else {
                            break;
                        }
                    }
                }
                (i == addr.len()).then(|| addr.len() as u32)
            }
            _ => panic!("filewrite"),
        }
        .map(|x| x as usize)
    }
}

/// major device番号をデバイス用関数にマップする
pub struct DevSw {
    pub read: OnceCell<fn(bool, ARange<VAddr>) -> Option<usize>>,
    pub write: OnceCell<fn(bool, ARange<VAddr>) -> Option<usize>>,
}
impl DevSw {
    pub fn read(&self, user_dst: bool, dst: ARange<VAddr>) -> Option<u32> {
        self.read
            .get()
            .and_then(|f| f(user_dst, dst))
            .map(|x| x as u32)
    }
    pub fn write(&self, user_src: bool, src: ARange<VAddr>) -> Option<u32> {
        self.write
            .get()
            .and_then(|f| f(user_src, src))
            .map(|x| x as u32)
    }
}
unsafe impl Sync for DevSw {}
pub static DEV_SW: [DevSw; NDEV as usize] = [const {
    DevSw {
        read: OnceCell::new(),
        write: OnceCell::new(),
    }
}; NDEV as usize];
pub const CONSOLE: usize = 1;

struct FTable {
    file: [File; NFILE],
}
static FTABLE_LOCK: SpinLock<FTable> = SpinLock::new(
    FTable {
        file: [const { File::new() }; NFILE],
    },
    "ftable",
);

pub struct FileHandle {
    file: NonNull<File>,
}
impl Deref for FileHandle {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        unsafe { self.file.as_ref() }
    }
}
impl DerefMut for FileHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.file.as_mut() }
    }
}
impl Clone for FileHandle {
    /// filedup
    fn clone(&self) -> Self {
        let ftable = FTABLE_LOCK.acquire();
        assert!(self.ref_cnt >= 1, "file_dup");
        unsafe {
            (*self.file.as_ptr()).ref_cnt += 1;
        }
        Self { file: self.file }
    }
}
impl Drop for FileHandle {
    /// fileclose
    fn drop(&mut self) {
        let ftable = FTABLE_LOCK.acquire();
        assert!(self.ref_cnt >= 1, "file_close");
        self.ref_cnt -= 1;
        if self.ref_cnt > 0 {
            return;
        }

        let mut ff = unsafe { core::mem::replace(self.file.as_mut(), File::new()) };
        drop(ftable);

        match ff.file_type {
            FileType::FdPipe => unsafe { (*ff.pipe).close(ff.writable) },
            FileType::FdInode | FileType::FdDevice => {
                let handle = LogHandle::new();
                unsafe {
                    ff.ip.drop(&handle);
                }
            }
            _ => {}
        }
    }
}

impl FileHandle {
    /// filealloc
    pub fn new() -> Option<Self> {
        let mut ftable = FTABLE_LOCK.acquire();
        ftable.file.iter_mut().find(|f| f.ref_cnt == 0).map(|f| {
            f.ref_cnt = 1;
            Self {
                file: NonNull::from_mut(f),
            }
        })
    }
    /// fのメタデータを取得
    pub fn state(&mut self) -> Option<FileState> {
        let p = my_proc();
        matches!(self.file_type, FileType::FdInode | FileType::FdDevice).then(|| unsafe {
            let mut guard = self.ip.lock();
            let ret = guard.state();
            self.ip.unlock();
            ret
        })
    }
}
