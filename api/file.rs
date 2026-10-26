use crate::param::*;

#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u16)]
pub enum InodeType {
    Free = 0,
    Directory = 1,
    File = 2,
    Device = 3,
}
impl InodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Directory => "Directory",
            Self::File => "File",
            Self::Device => "Device",
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct FileOpenMode(pub u32);
impl FileOpenMode {
    pub const RD_ONLY: Self = Self(0x000);
    pub const WR_ONLY: Self = Self(0x001);
    pub const RDWR: Self = Self(0x002);
    pub const CREATE: Self = Self(0x200);
    pub const TRUNC: Self = Self(0x400);
    pub fn bits(&self) -> u32 {
        self.0
    }
}
impl core::ops::BitOr for FileOpenMode {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitAnd for FileOpenMode {
    type Output = bool;
    fn bitand(self, rhs: Self) -> bool {
        self.0 & rhs.0 != 0
    }
}

/// stat.cから移植
#[repr(C)]
pub struct FileState {
    pub dev: u32,              // ファイルシステムのディスクデバイス
    pub inum: u32,             // inode番号
    pub inode_type: InodeType, // inodeタイプ
    pub nlink: u16,            // ファイルへのリンク数
    pub size: u32,             // ファイルサイズ
}
impl FileState {
    pub const fn new() -> Self {
        Self {
            dev: 0,
            inum: 0,
            inode_type: InodeType::Free,
            nlink: 0,
            size: 0,
        }
    }
}

/// name filedはDIRSIZ文字持ち、NULLで終わらない
#[repr(C)]
pub struct DirEntry {
    pub inum: u32,
    pub name: [u8; DIR_SZ],
}
impl DirEntry {
    pub const fn new() -> Self {
        Self {
            inum: 0,
            name: [0; DIR_SZ],
        }
    }
}
