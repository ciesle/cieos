use core::any::Any;
use core::cell::OnceCell;
use core::ops::{Deref, DerefMut};

use super::{buf::*, inode::*, log::*};
use crate::proc::proc::*;
use crate::utils::printk::*;

use api::file::*;
use api::param::*;
use api::vm::*;

// ディスク上のファイルシステム
// カーネルとユーザどちらもこのファイルを使う

// Disk layout
// [ boot block | super block | log | inode blocks | free bit map | data blocks ]

// ファイルシステムは5つのレイヤーからなる
// Blocks：素のディスクブロックのアロケータ
// Log：複数ステップの更新におけるクラッシュ対策
// Files：inodeが割り当てられる。読み書き可能。
// Directories：inodeが割り当てられる。他のinodeを保存する
// Names：パス

/// mkfsがsuperblockを求め、ファイルシステムを初期化する
#[repr(C)]
pub struct SuperBlockInner {
    pub magic: u32,       // FSMAGIC
    pub size: u32,        // ファイルシステムイメージの大きさ / block
    pub nblocks: u32,     // データブロックの数
    pub ninodes: u32,     // inodeの数
    pub nlog: u32,        // logブロックの数
    pub log_start: u32,   // 最初のログブロックの番号
    pub inode_start: u32, // 最初のinodeブロックの番号
    pub bmap_start: u32,  // 足所のfree mapブロックの番号
}
pub struct SuperBlock {
    data: OnceCell<SuperBlockInner>,
}
impl Deref for SuperBlock {
    type Target = SuperBlockInner;
    fn deref(&self) -> &Self::Target {
        &self.data.get().unwrap()
    }
}
unsafe impl Sync for SuperBlock {}
pub static SB: SuperBlock = SuperBlock {
    data: OnceCell::new(),
};

const FSMAGIC: u32 = 0x10203040;

/// ここからは、低レベルなファイルシステム操作のための関数を記述する。
/// 高レベルなAPIはsysfile.rsにある。
pub fn fs_init(dev: u32) {
    // read_sb
    unsafe {
        let (b, guard) = bread(dev, 1);
        let mut sb: SuperBlockInner = SuperBlockInner {
            magic: 0,
            size: 0,
            nblocks: 0,
            ninodes: 0,
            nlog: 0,
            log_start: 0,
            inode_start: 0,
            bmap_start: 0,
        };
        core::ptr::copy_nonoverlapping(
            guard.data.as_ptr(),
            &raw mut sb as *mut u8,
            size_of::<SuperBlockInner>(),
        );
        SB.data.set(sb);
    }
    assert!(SB.magic == FSMAGIC, "invalid file system");

    init_log(dev, &SB);
    InodeHandle::reclaim(dev);
}

// ディレクトリ操作

/// ディレクトリエントリを探す
/// 見つかった場合、Inodeとエントリのバイトオフセットを返す
pub fn dir_lookup(
    name: &[u8],
    guard: &mut InodeGuard,
    handle: &LogHandle,
) -> Option<(InodeHandle, u32)> {
    assert!(
        guard.inode_type == InodeType::Directory,
        "dirlookup not DIR"
    );
    let mut de: DirEntry = DirEntry::new();
    (0..guard.size)
        .step_by(size_of::<DirEntry>())
        .find(|&off| unsafe {
            unsafe {
                let tot = guard.read(
                    false,
                    ARange::from_mut_ref(&mut de),
                    off,
                    size_of::<DirEntry>() as u32,
                );
                assert_eq!(tot, Some(size_of::<DirEntry>() as u32), "dirlookup read");
                de.inum != 0
                    && name
                        == &de.name[..de
                            .name
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(de.name.len())]
            }
        })
        .map(|off| (InodeHandle::get(unsafe { guard.dev }, de.inum), off))
}

/// dpに新しいディレクトリエントリを作る
/// 成功したかを返す
pub fn dir_link(name: &[u8], inum: u32, guard: &mut InodeGuard, handle: &LogHandle) -> bool {
    // 同じ名前のファイルがあるとエラー
    if let Some((mut ip, _)) = dir_lookup(name, guard, handle) {
        ip.drop(handle);
        return false;
    }
    let mut de: DirEntry = DirEntry::new();
    // 空のエントリを探す
    let off = (0..guard.size)
        .step_by(size_of::<DirEntry>())
        .find(|&off| {
            let tot = guard.read(
                false,
                ARange::from_mut_ref(&mut de),
                off,
                size_of::<DirEntry>() as u32,
            );
            assert_eq!(tot, Some(size_of::<DirEntry>() as u32), "dirlink read");
            de.inum == 0
        })
        .unwrap_or(guard.size);
    de.name = [0u8; DIR_SZ];
    de.name[..name.len()].copy_from_slice(name);
    de.inum = inum;
    let tot = guard.write(
        false,
        ARange::from_ref(&de),
        off,
        size_of::<DirEntry>() as u32,
        handle,
    );
    tot == Some(size_of::<DirEntry>() as u32)
}

// パス
// 次のパス要素をpathからnameにコピーする
// 続く要素の対するポインタを返す
// pathが空かどうかで最後の要素化を判定できる

/// 例
///   skipelem("a/bb/c", name) = "bb/c", setting name = "a"
///   skipelem("///a//bb", name) = "bb", setting name = "a"
///   skipelem("a", name) = "", setting name = "a"
///   skipelem("", name) = skipelem("////", name) = 0
fn skip_elem(mut path: &[u8]) -> Option<(&[u8], &[u8])> {
    path = &path[..path.iter().position(|&c| c == 0).unwrap_or(path.len())];
    while path.starts_with(b"/") {
        path = &path[1..];
    }
    if path.is_empty() {
        return None;
    }
    let end = path.iter().position(|&c| c == b'/').unwrap_or(path.len());
    let name = &path[..end.min(DIR_SZ)];

    path = &path[end..];
    while path.starts_with(b"/") {
        path = &path[1..];
    }
    Some((name, path))
}

/// path nameのinodeを検索する
/// nameiparentがtrueなら、親のinodeを返し、名前をnameに入れる
fn namex<'a>(
    mut path: &'a [u8],
    nameiparent: bool,
    handle: &LogHandle,
) -> Option<(InodeHandle, &'a [u8])> {
    let mut ip = match path.first() {
        Some(b'/') => InodeHandle::get(ROOT_DEV, ROOTINO),
        _ => unsafe { (*my_proc().local.get()).cwd.clone() },
    };
    let mut last_name: &[u8] = &[];
    while let Some((name, rest)) = skip_elem(path) {
        path = rest;
        last_name = name;

        unsafe {
            let mut guard = ip.lock();
            if guard.inode_type != InodeType::Directory {
                ip.drop(handle);
                return None;
            }
            if guard.nlink == 0 {
                ip.drop(handle);
                return None;
            }
            if nameiparent && path.is_empty() {
                ip.unlock();
                return Some((ip, name));
            }
            match dir_lookup(name, guard, handle) {
                Some((next, _)) => {
                    ip.drop(handle);
                    ip = next;
                }
                None => {
                    ip.drop(handle);
                    return None;
                }
            }
        }
    }
    if nameiparent {
        unsafe {
            ip.drop(handle);
        }
        None
    } else {
        Some((ip, last_name))
    }
}

pub fn namei(path: &[u8], handle: &LogHandle) -> Option<InodeHandle> {
    namex(path, false, handle).map(|x| x.0)
}

pub fn nameiparent<'a>(path: &'a [u8], handle: &LogHandle) -> Option<(InodeHandle, &'a [u8])> {
    namex(path, true, handle)
}
