use core::ops::{Deref, DerefMut};
use core::ptr::{null, null_mut};

use super::{fs::*, log::*, virtio::*};
use crate::{
    proc::{sleeplock::*, spinlock::*},
    utils::printk::*,
};

use api::param::*;

pub struct BufContent {
    pub valid: bool, // diskから読みだしたデータがあるか？
    pub parent: usize,
    pub data: [u8; BLOCK_SIZE as usize],
}
pub struct Buf {
    pub disk: bool, // diskはbufを所有しているか？
    pub dev: u32,
    pub blockno: u32,
    pub ref_cnt: u32,
    pub prev: *mut Buf,
    pub next: *mut Buf,
    pub lock: SleepLock<BufContent>,
}

impl Buf {
    pub const fn new() -> Self {
        Self {
            disk: false,
            dev: 0,
            blockno: 0,
            ref_cnt: 0,
            prev: null_mut(),
            next: null_mut(),
            lock: SleepLock::new(
                BufContent {
                    valid: false,
                    parent: 0,
                    data: [0; BLOCK_SIZE as usize],
                },
                "bufdata",
            ),
        }
    }
}

pub struct BufGuard {
    pub b: *mut Buf,
    pub guard: Option<Guard<'static, BufContent, SleepLock<BufContent>>>,
}
impl Deref for BufGuard {
    type Target = BufContent;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.guard.as_deref().unwrap() }
    }
}
impl DerefMut for BufGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.guard.as_deref_mut().unwrap() }
    }
}

/// Buffer cache
///
/// Buffer cacheは、ディスクブロックのコピーを持つBuf構造体の連結リストである。
/// ディスクブロックをキャッシュすることで、読み書き回数を減らし、
/// 複数操作についての書き込みの同期点を作れる
///
/// bread: 特定のディスクブロックのbufferを得る
/// bwrite: bufferをディスクに書き戻す
/// brelease: バッファを開放する
/// 1つのバッファは同時に1つのプロセスからしかアクセスできない

struct BCache {
    buf: [Buf; NBUF as usize],
    // すべてのバッファの連結リスト。nextが最も新しく、prevが最も古い
    // headを番兵として持つ
    head: Buf,
}

static BCACHE_LOCK: SpinLock<BCache> = SpinLock::new(
    BCache {
        buf: [const { Buf::new() }; NBUF as usize],
        head: Buf::new(),
    },
    "bcache",
);

pub fn binit() {
    let mut bcache = BCACHE_LOCK.acquire();

    let BCache { buf, head } = &mut *bcache;

    // バッファの連結リストを作る
    head.prev = &raw mut *head;
    head.next = &raw mut *head;
    for (i, b) in (*buf).iter_mut().enumerate() {
        b.next = head.next;
        b.prev = &raw mut *head;
        unsafe {
            (*head.next).prev = &raw mut *b;
            head.next = &raw mut *b;
        }
        unsafe {
            let mut data = b.lock.acquire_without_lock();
            (*data).parent = i;
        }
    }
}

/// buffer cacheを参照し、(dev, blockno)のキャッシュがあるか探索。
/// なければ割り当てて返す
/// この関数は割り当てるだけで、読み込みや書き込みは行わない
pub fn bget(dev: u32, blockno: u32) -> (*mut Buf, BufGuard) {
    let mut bcache = BCACHE_LOCK.acquire();

    let mut b = bcache.head.next;
    // ブロックはキャッシュされてる？
    while b != &raw mut bcache.head {
        unsafe {
            if (*b).dev == dev && (*b).blockno == blockno {
                (*b).ref_cnt += 1;
                drop(bcache);
                return (
                    b,
                    BufGuard {
                        b: b,
                        guard: Some((*b).lock.acquire()),
                    },
                );
            }
            b = (*b).next;
        }
    }

    // ブロックはキャッシュされていなかった
    // 最も昔に開放されたエントリを取得する
    b = bcache.head.prev;
    while b != &raw mut bcache.head {
        unsafe {
            if (*b).ref_cnt == 0 {
                (*b).dev = dev;
                (*b).blockno = blockno;
                (*b).ref_cnt = 1;
                // 通常、bcacheのスピンロックを取得している最中に、
                // sleeplockを取得してはならない（sleepに入る可能性があるため）
                // 今回は、refcntが0であることから、このロックにアクセスされることはない。
                let mut data = (*b).lock.acquire();
                data.valid = false;
                return (
                    b,
                    BufGuard {
                        b: b,
                        guard: Some(data),
                    },
                );
            }
            b = (*b).prev;
        }
    }
    panic!("bget: no buffers");
}

/// blockの内容をキャッシュしたbufを返す
pub fn bread(dev: u32, blockno: u32) -> (*mut Buf, BufGuard) {
    let (b, mut data) = bget(dev, blockno);
    if !data.valid {
        virtio_disk_rw(b, &mut data, false);
        data.valid = true;
    }
    (b, data)
}

/// bの内容をdiskに書き込む
pub fn bwrite(b: *mut Buf, data: &mut BufGuard) {
    virtio_disk_rw(b, data, true);
}

// bufferのロックを開放する
// LRUリストに追加
// brelseに該当
impl Drop for BufGuard {
    fn drop(&mut self) {
        drop(self.guard.take());

        let mut bcache = BCACHE_LOCK.acquire();
        unsafe {
            (*self.b).ref_cnt -= 1;
            // 誰も使っていない
            if (*self.b).ref_cnt == 0 {
                (*(*self.b).next).prev = (*self.b).prev;
                (*(*self.b).prev).next = (*self.b).next;
                (*self.b).next = bcache.head.next;
                (*self.b).prev = &raw mut bcache.head;
                (*bcache.head.next).prev = self.b;
                bcache.head.next = self.b;
            }
        }
    }
}

pub fn bpin(b: *mut Buf) {
    let bcache = BCACHE_LOCK.acquire();
    unsafe {
        (*b).ref_cnt += 1;
    }
}

pub fn bunpin(b: *mut Buf) {
    let bcache = BCACHE_LOCK.acquire();
    unsafe {
        (*b).ref_cnt -= 1;
    }
}

/// ブロックをゼロ埋め
fn bzero(dev: u32, blockno: u32, handle: &LogHandle) {
    let (b, mut guard) = bread(dev, blockno);
    guard.data = [0u8; BLOCK_SIZE as usize];
    handle.write(b);
}

/// bitmap bを格納するブロック番号
fn bitmap_block(b: u32, sb: &SuperBlock) -> u32 {
    b / BITMAP_PER_BLOCK + sb.bmap_start
}

/// ゼロ埋めされたディスクブロックを割り当てる
pub fn balloc(dev: u32, handle: &LogHandle) -> Option<u32> {
    for b in (0..SB.size).step_by(BITMAP_PER_BLOCK as usize) {
        let (bp, mut bguard) = bread(dev, bitmap_block(b, &SB));
        for bi in (0..BITMAP_PER_BLOCK).take_while(|&bi| b + bi < SB.size) {
            let m = 1 << (bi % 8);
            if (bguard.data[(bi / 8) as usize] & m) == 0 {
                bguard.data[(bi / 8) as usize] |= m;
                handle.write(bp);
                // bzeroは内部でbreadを呼ぶ。そのため、bpのロックも持った状態で
                // bzeroを呼び出すと、デッドロックになりうる。
                drop(bguard);
                bzero(dev, b + bi, handle);
                return Some(b + bi);
            }
        }
    }
    printk!("balloc: out of blocks");
    None
}

/// ディスクブロックを解放
pub fn bfree(dev: u32, b: u32, handle: &LogHandle) {
    let (bp, mut bguard) = bread(dev, bitmap_block(b, &SB));
    let bi = b % BITMAP_PER_BLOCK;
    let m = 1 << (bi % 8);
    assert!(
        (bguard.data[(bi / 8) as usize] & m) != 0,
        "freeing free block"
    );
    bguard.data[(bi / 8) as usize] &= !m;
    handle.write(bp);
}
