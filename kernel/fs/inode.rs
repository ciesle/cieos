use core::ops::{Deref, DerefMut};
use core::ptr::{null, null_mut};

use super::{buf::*, fs::*, log::*};
use crate::dev::uart::uart_putc_sync;
use crate::{
    proc::{proc::*, sleeplock::*, spinlock::*},
    utils::printk::*,
};

use api::file::*;
use api::param::*;
use api::vm::*;

/* inode
inodeは、一つの無名ファイルを表現する。
inodeのディスク構造体は、ファイルタイプ、サイズ、リンク数、ブロックリストを持つ。

inodeはsb.inodestartから順番に並ぶ。各inodeは、ディスク上の位置に応じて番号をつけられる

kernelは、使用中のinodeをメモリ中に保存し、inodeへの同期アクセスを可能にしている。
メモリ中のinodeには、現在使用されているかを示すフィールドがある。

inodeとそのメモリ上表現は、ファイルシステム上で使えるようになるまでに変遷をたどる。

- 割当
  ディスクでのfile-typeがnonzeroならinodeは割り当てられている。
  iallocが割当、iputが解放する

- テーブルでの参照
  inode tableのカウントが0なら解放されている。
  そうでないなら、メモリ上のポインタから参照されている数を示す

- 有効か
  inode table entryの情報が正しいか
  ilockがinodeをディスクから読む

- ロックされている
  ファイルシステムはロックされている場合にのみinodeの中身を読む

典型的な処理は以下のようになる
  ip = iget(dev, inum)
  ilock(ip)
  ... inodeに関する処理 ...
  iunlock(ip)
  iput(ip)

システムコールがinodeに対して長期の参照を持てるように、ilockとigetは分離されている。
ロックはreadなどの短期のアクセスでのみ使う
パスネーム探索のときのデッドロックやレースを回避する狙いもある。
igetはref_cntを増やし、inode tableから消えないようにする

多くのファイルシステム関数は、呼び出し元がロックをとっていることを期待する。
これにより、システムコールによる複数ステップのアトミック動作を可能にする。

itable.lockは、itableの割り当てを保護する。ip.ref_cntがエントリが空かを示し、
ip.devとip.numが位置を持つ。これらのフィールドを捜査するには、itable.lockが必要

ip.lockはref_cnt, dev, inum以外の値を保護する。
ip.valid, size, typeなどを編集するにはip.lockが必要

*/
const INODE_PER_BLOCK: u32 = BLOCK_SIZE / size_of::<DInode>() as u32; // ブロックごとのinode数

/// ディスク上のinode構造体
#[repr(C)]
struct DInode {
    inode_type: InodeType,              // file type
    major: u16,                         // major device number
    minor: u16,                         // minor device number
    nlink: u16,                         // inodeへのリンク数
    size: u32,                          // ファイルサイズ / byte
    addrs: [u32; NDIRECT as usize + 1], // データブロックのアドレス
}
impl DInode {
    pub const fn new() -> Self {
        Self {
            inode_type: InodeType::Free,
            major: 0,
            minor: 0,
            nlink: 0,
            size: 0,
            addrs: [0; NDIRECT as usize + 1],
        }
    }
}

/// inode iを格納するブロック番号
fn inode_block(i: u32, sb: &SuperBlock) -> u32 {
    i / INODE_PER_BLOCK + sb.inode_start
}
pub struct InodeContent {
    pub valid: bool,
    pub inode_type: InodeType,
    pub major: u16,
    pub minor: u16,
    pub nlink: u16,
    pub size: u32,
    addrs: [u32; NDIRECT as usize + 1],
}
pub struct Inode {
    pub dev: u32,   // デバイス数
    pub inum: u32,  // inode
    ref_cnt: usize, // 参照数
    lk: SleepLock<InodeContent>,
}
impl Inode {
    pub const fn new() -> Self {
        Self {
            dev: 0,
            inum: 0,
            ref_cnt: 0,
            lk: SleepLock::new(
                InodeContent {
                    valid: false,
                    inode_type: InodeType::Free,
                    major: 0,
                    minor: 0,
                    nlink: 0,
                    size: 0,
                    addrs: [0; NDIRECT as usize + 1],
                },
                "inode",
            ),
        }
    }
}

struct InodeTable {
    table: [Inode; NINODE as usize],
}
static INODE_TABLE: SpinLock<InodeTable> = SpinLock::new(
    InodeTable {
        table: [const { Inode::new() }; NINODE as usize],
    },
    "inode_table",
);

pub struct InodeHandle {
    ip: *mut Inode,
    guard: Option<InodeGuard>,
}
impl Deref for InodeHandle {
    type Target = Inode;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ip }
    }
}
impl DerefMut for InodeHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ip }
    }
}
impl Clone for InodeHandle {
    /// dup
    /// ipのref_cntを増やす
    /// ip = ip.dup() をできるように、ipを返す
    /// 並行で呼ばれる可能性があるので、&mut selfを持てない
    fn clone(&self) -> Self {
        let state = INODE_TABLE.acquire();
        unsafe {
            (*self.ip).ref_cnt += 1;
            Self {
                ip: self.ip,
                guard: None,
            }
        }
    }
}
impl InodeHandle {
    pub const fn new() -> Self {
        Self {
            ip: null_mut(),
            guard: None,
        }
    }
    /// 与えられたinumについて、メモリ上のinodeキャッシュに該当があるかを検索
    /// なければ、キャッシュの空きを割り当てる
    /// ディスクから中身をキャッシュすることはない
    pub fn get(dev: u32, inum: u32) -> Self {
        let mut itable = INODE_TABLE.acquire();
        let mut empty: Option<&mut Inode> = None;
        for ip in &mut itable.table {
            if ip.ref_cnt > 0 && ip.dev == dev && ip.inum == inum {
                ip.ref_cnt += 1;
                return Self {
                    ip: core::ptr::from_mut(ip),
                    guard: None,
                };
            }
            if empty.is_none() && ip.ref_cnt == 0 {
                empty = Some(ip);
            }
        }
        match empty {
            Some(ip) => {
                ip.dev = dev;
                ip.inum = inum;
                ip.ref_cnt = 1;
                unsafe {
                    let mut guard = ip.lk.acquire_without_lock();
                    (*guard).valid = false;
                }
                Self {
                    ip: core::ptr::from_mut(ip),
                    guard: None,
                }
            }
            None => panic!("iget: no inodes"),
        }
    }

    /// device devのinodeを1つ新規に割り当てる
    /// それをinode_typeとして割り当てられたとして記録し、ロックされていないが、
    /// referenceされたinodeを返す。
    pub fn alloc(dev: u32, inode_type: InodeType, handle: &LogHandle) -> Option<Self> {
        for inum in 1..SB.ninodes {
            let (bp, mut bguard) = bread(dev, inode_block(inum, &SB));
            unsafe {
                let dip = bguard
                    .data
                    .as_mut_ptr()
                    .cast::<DInode>()
                    .add((inum % INODE_PER_BLOCK) as usize);
                if (*dip).inode_type == InodeType::Free {
                    (*dip) = DInode::new();
                    (*dip).inode_type = inode_type;
                    handle.write(bp);
                    drop(bguard);
                    return Some(Self::get(dev, inum));
                }
            }
        }
        printk!("inode alloc: no inodes\n");
        None
    }

    /// put / unlockputに相当
    /// メモリ上inodeに対する参照を減らす
    /// これが最後の参照なら、inode tableはエントリを再利用できる
    /// 最後の参照で、inodeがリンクを持っていなければ、ディスク上のものも解放できる
    /// トランザクションの内側でしか呼んではいけない。
    pub fn drop(mut self, handle: &LogHandle) {
        self.guard = None; // unlock

        let mut state = INODE_TABLE.acquire();
        // inodeはリンクも参照もない。消して良い
        unsafe {
            if self.ref_cnt == 1 {
                // ip->ref_cntが1の場合、他に参照するプロセスが存在しないので、sleep lockをとっても
                // ブロックすることはない
                // nlinkも0なら、これから新しく参照されることもない
                let mut guard = self.lock();
                if guard.valid && guard.nlink == 0 {
                    drop(state);
                    guard.trunc(handle);
                    guard.inode_type = InodeType::Free;
                    guard.update(handle);
                    guard.valid = false;
                    self.unlock();
                    state = INODE_TABLE.acquire();
                }
            }
            self.ref_cnt -= 1;
        }
    }

    /// 与えられたinodeをロックする
    /// もし必要ならディスクから読む
    pub fn lock(&mut self) -> &mut InodeGuard {
        assert!(self.ref_cnt >= 1, "ilock");
        let mut data = unsafe { (*self.ip).lk.acquire() };
        if !data.valid {
            unsafe {
                let (bp, mut bguard) = bread(self.dev, inode_block(self.inum, &SB));
                let dip = bguard
                    .data
                    .as_mut_ptr()
                    .cast::<DInode>()
                    .add((self.inum % INODE_PER_BLOCK) as usize)
                    as *mut DInode;
                data.inode_type = (*dip).inode_type;
                data.major = (*dip).major;
                data.minor = (*dip).minor;
                data.nlink = (*dip).nlink;
                data.size = (*dip).size;
                data.addrs = (*dip).addrs;
            }
            data.valid = true;
            assert!(data.inode_type != InodeType::Free, "ilock: no type");
        }
        self.guard = Some(InodeGuard {
            guard: data,
            dev: self.dev,
            inum: self.inum,
        });
        self.guard.as_mut().unwrap()
    }
    pub fn acquire_without_lock(&mut self) -> &mut InodeGuard {
        self.guard.as_mut().unwrap()
    }
    pub fn unlock(&mut self) {
        self.guard = None;
    }

    /// ディスク上のinodeブロックを全て走査し、孤児が見つかったら消す
    pub fn reclaim(dev: u32) {
        for inum in 1..SB.ninodes {
            if let Some(mut ip) = unsafe {
                let (bp, mut bguard) = bread(dev, inode_block(inum, &SB));
                let dip = bguard
                    .data
                    .as_mut_ptr()
                    .cast::<DInode>()
                    .add((inum % INODE_PER_BLOCK) as usize);
                // 孤児の検出
                if (*dip).inode_type != InodeType::Free && (*dip).nlink == 0 {
                    printk!("ireclaim: orphaned inode {}\n", inum);
                    Some(InodeHandle::get(dev, inum))
                } else {
                    None
                }
            } {
                unsafe {
                    let handle = LogHandle::new();
                    let mut guard = ip.lock(); // ディスクから取得して
                    ip.drop(&handle);
                    // ここでのhandle.dropによりディスクに書き戻す処理をやり直させる
                }
            }
        }
    }
}

pub struct InodeGuard {
    guard: Guard<'static, InodeContent, SleepLock<InodeContent>>,
    pub dev: u32,
    pub inum: u32,
}
impl Deref for InodeGuard {
    type Target = InodeContent;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.guard }
    }
}
impl DerefMut for InodeGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.guard }
    }
}
impl InodeGuard {
    /// メモリ上の変更されたinodeをディスクにコピーする
    /// 全てのinodeへの変更がディスク上に移された後に行う
    pub fn update(&self, handle: &LogHandle) {
        let (bp, mut bguard) = bread(self.dev, inode_block(self.inum, &SB));
        unsafe {
            let dip = bguard
                .data
                .as_mut_ptr()
                .cast::<DInode>()
                .add((self.inum % INODE_PER_BLOCK) as usize);
            (*dip).inode_type = self.inode_type;
            (*dip).major = self.major;
            (*dip).minor = self.minor;
            (*dip).nlink = self.nlink;
            (*dip).size = self.size;
            (*dip).addrs = self.addrs;
            handle.write(bp);
        }
    }

    /// inodeのデータは、最初のNDIRECTブロックはip.addrsに直に、
    /// その次のNINDIRECTブロックはip.addrs[NDIRECT]にしまわれる。
    /// n番目のブロックのディスクブロックアドレスを返す。
    /// もしなければ割り当てる
    fn bmap(&mut self, mut bn: u32, handle: &LogHandle) -> Option<u32> {
        if bn < NDIRECT {
            let mut addr = self.addrs[bn as usize];
            if addr == 0 {
                addr = balloc(self.dev, handle)?;
                self.addrs[bn as usize] = addr;
            }
            return Some(addr);
        }

        bn -= NDIRECT;
        assert!(bn < NINDIRECT, "bmap: out of range");

        let mut addr = self.addrs[NDIRECT as usize];
        if addr == 0 {
            addr = balloc(self.dev, handle)?;
            self.addrs[NDIRECT as usize] = addr;
        }
        let (bp, mut bguard) = bread(self.dev, addr);
        let mut data = bguard.data.as_ptr() as *mut [u32; (BLOCK_SIZE / (32 / 8)) as usize];
        unsafe {
            addr = (*data)[bn as usize];
            if addr == 0 {
                addr = balloc(self.dev, handle)?;
                (*data)[bn as usize] = addr;
                handle.write(bp);
            }
        }
        Some(addr)
    }

    /// アクセスするアドレスが必ず割り当てられていると保証できる場合
    fn bmap_without_alloc(&self, mut bn: u32) -> u32 {
        if bn < NDIRECT {
            assert!(
                self.addrs[bn as usize] != 0,
                "bmap_without_alloc: not mapped"
            );
            return self.addrs[bn as usize];
        }

        bn -= NDIRECT;
        assert!(bn < NINDIRECT, "bmap: out of range");

        let mut addr = self.addrs[NDIRECT as usize];
        assert!(addr != 0, "bmap_without_alloc: not mapped");
        let (bp, mut bguard) = bread(self.dev, addr);
        let mut data = bguard.data.as_ptr() as *mut [u32; (BLOCK_SIZE / (32 / 8)) as usize];
        unsafe {
            addr = (*data)[bn as usize];
        }
        assert!(addr != 0, "bmap_without_alloc: not mapped");
        addr
    }

    /// inodeからコンテンツを切り詰める
    /// 割り当てられたブロックを全て開放する
    pub fn trunc(&mut self, handle: &LogHandle) {
        for i in 0..NDIRECT {
            if self.addrs[i as usize] != 0 {
                bfree(self.dev, self.addrs[i as usize], handle);
                self.addrs[i as usize] = 0;
            }
        }
        if self.addrs[NDIRECT as usize] != 0 {
            let (bp, mut bguard) = bread(self.dev, self.addrs[NDIRECT as usize]);
            let mut data = bguard.data.as_ptr() as *mut [u32; (BLOCK_SIZE / (32 / 8)) as usize];
            for j in 0..NINDIRECT {
                unsafe {
                    if (*data)[j as usize] != 0 {
                        bfree(self.dev, (*data)[j as usize], handle);
                    }
                }
            }
            drop(bguard);
            bfree(self.dev, self.addrs[NDIRECT as usize], handle);
            self.addrs[NDIRECT as usize] = 0;
        }
        self.size = 0;
        self.update(handle);
    }

    pub fn state(&self) -> FileState {
        FileState {
            dev: self.dev,
            inum: self.inum,
            inode_type: self.inode_type,
            nlink: self.nlink,
            size: self.size,
        }
    }

    /// inodeからデータを読み込む
    /// user_dstがtrueならuser、falseならkernel
    /// dstからnの範囲は割り当てられていると保証する
    pub fn read(
        &self,
        user_dst: bool,
        mut dst: ARange<VAddr>,
        mut off: u32, // 書き込み元のoffset
        mut n: u32,   // 書き込みサイズ
    ) -> Option<u32> {
        assert!(dst.len() == n as usize);
        if off > self.size || off + n < off {
            return Some(0);
        }
        if off + n > self.size {
            n = self.size - off;
        }

        let mut tot = 0;
        while tot < n {
            let addr = self.bmap_without_alloc(off / BLOCK_SIZE);
            let (bp, mut bguard) = bread(self.dev, addr);
            let m = core::cmp::min(n - tot, BLOCK_SIZE - off % BLOCK_SIZE);
            if !either_copyout(
                user_dst,
                ARange::span(dst.start, m as usize),
                &bguard.data[(off % BLOCK_SIZE) as usize..(off % BLOCK_SIZE + m) as usize],
            ) {
                return None;
            }
            tot += m;
            off += m;
            dst.start = dst.start + m as usize;
        }
        Some(tot)
    }

    /// データをinodeに書き込む
    /// user_srcがtrueならuser空間から、違うならkernel空間から
    /// 書き込めたバイト数を返す
    pub fn write(
        &mut self,
        user_src: bool,
        mut src: ARange<VAddr>, // 書き込み元のアドレス
        mut off: u32,           // destのオフセット
        n: u32,                 // 書き込みバイト数
        handle: &LogHandle,
    ) -> Option<u32> {
        assert!(src.len() == n as usize);
        if off > self.size || off + n < off {
            return None;
        }
        if off + n > MAX_FILE * BLOCK_SIZE {
            return None;
        }
        let mut tot = 0;
        while tot < n {
            let addr = match self.bmap(off / BLOCK_SIZE, handle) {
                None => break,
                Some(addr) => addr,
            };
            let (bp, mut bguard) = bread(self.dev, addr);
            let m = core::cmp::min(n - tot, BLOCK_SIZE - off % BLOCK_SIZE);
            if !either_copyin(
                &mut bguard.data[(off % BLOCK_SIZE) as usize..(off % BLOCK_SIZE + m) as usize],
                user_src,
                ARange::span(src.start, m as usize),
            ) {
                // 失敗していても、部分的に成功しているかもしれない
                handle.write(bp);
                break;
            }
            handle.write(bp);
            tot += m;
            off += m;
            src.start = src.start + m as usize;
        }
        if off > self.size {
            self.size = off;
        }

        // inodeをディスクに書き戻す
        // bmapを呼び、新しいip->addrsを作ったかもしれないため
        self.update(handle);
        Some(tot)
    }
}
