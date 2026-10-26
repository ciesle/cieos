//
// virtioのデバイス定義
// mmioとvirtio descriptorの両方で使う
// https://sycons.wuu1228.github.io/howto_implement_hypervisor/part11.html
//

use core::ptr::null;
use core::ptr::null_mut;

use super::buf::*;
use crate::{
    memlayout::*,
    proc::{kalloc::*, proc::*, spinlock::*},
    riscv::*,
};

use api::param::*;
use api::vm::*;

// virtio mmio control register
// 0x10_001_000 二マップされている
const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000; // 0x74726976 = virt
const VIRTIO_MMIO_VERSION: usize = 0x004; // バージョン 2
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008; // デバイスタイプ：1はnet, 2はディスク
const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c; // 0x554d4551 = QEMU
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030; // write-onlyなキュー
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034; // 現在のキューの最大サイズ
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038; // 現在のキューのサイズ
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044; // ready bit
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;
const VIRTIO_MMIO_STATUS: usize = 0x070;
const VIRTIO_MMIO_QUEUE_DESC_LOW: usize = 0x080; // descriptor tableの物理アドレス。write only
const VIRTIO_MMIO_QUEUE_DESC_HIGH: usize = 0x084;
const VIRTIO_MMIO_DRIVER_DESC_LOW: usize = 0x090; // available ringの物理アドレス。write only
const VIRTIO_MMIO_DRIVER_DESC_HIGH: usize = 0x094;
const VIRTIO_MMIO_DEVICE_DESC_LOW: usize = 0x0a0; // used ringの物理アドレス。write only
const VIRTIO_MMIO_DEVICE_DESC_HIGH: usize = 0x0a4;

// status register bit
const VIRTIO_CONFIG_S_ACKNOWLEDGE: u32 = 1;
const VIRTIO_CONFIG_S_DRIVER: u32 = 2;
const VIRTIO_CONFIG_S_DRIVER_OK: u32 = 4;
const VIRTIO_CONFIG_S_FEATURES_OK: u32 = 8;

// device feature bit
const VIRTIO_BLK_F_RO: u32 = 5; // ディスクはread-only
const VIRTIO_BLK_F_SCSI: u32 = 7; // scsi command passthruに対応
const VIRTIO_BLK_F_FLUSH: u32 = 9; // cache flush command二対応
const VIRTIO_BLK_F_CONFIG_WCE: u32 = 11; // writeback mode available
const VIRTIO_BLK_F_MQ: u32 = 12; // 一つ以上のvqをサポート
const VIRTIO_F_ANY_LAYOUT: u32 = 27;
const VIRTIO_RING_F_INDIRECT_DESC: u32 = 28;
const VIRTIO_RING_F_EVENT_IDX: u32 = 29;

// virtio descriptorsの数
const NUM: usize = 8;

/// 一つのvirtio descriptor
#[repr(C)]
pub struct VirtqDesc {
    addr: usize,
    len: u32,
    flags: u16,
    next: u16,
}
const VRING_DESC_F_NEXT: u16 = 1; // 後にdescriptorが続く
const VRING_DESC_F_WRITE: u16 = 2; // デバイス側が書く
// 一度の要求で複数のdescriptorを使う時、nextでチェーンにする

/// ドライバが書き込み、デバイスが読めるようになったchainのring
#[repr(C)]
pub struct VirtqAvail {
    flags: u16,       // 常に0
    idx: u16,         // ドライバが次に書くring[idx]
    ring: [u16; NUM], // chain headのdescriptor番号
    unused: u16,
}

/// used ringの中の一要素
/// デバイスがドライバに完了したリクエストを報告する
#[repr(C)]
pub struct VirtqUsedElem {
    id: u32, // 完了したdescriptor chainの先頭
    len: u32,
}

#[repr(C)]
pub struct VirtqUsed {
    flags: u16, // 常に0
    idx: u16,   // ring[]のエントリを追加したら加算する
    ring: [VirtqUsedElem; NUM],
}

// virtio block devicesに特有の設定

const VIRTIO_BLK_T_IN: u32 = 0; // disk -> memory
const VIRTIO_BLK_T_OUT: u32 = 1; // memory -> disk

/// disk request内の最初のdescriptorの形式
/// このあとに2つのdescriptorが続き、ブロックと1バイトのステータスを保持する
#[repr(C)]
pub struct VirtioBlkReq {
    mode: u32, // VIRTIO_BLK_T_IN or OUT
    reserved: u32,
    sector: u64, // 読み書きを開始するセクタ番号
}
impl VirtioBlkReq {
    pub const fn new() -> Self {
        Self {
            mode: 0,
            reserved: 0,
            sector: 0,
        }
    }
}
pub struct DiskInfo {
    b: *mut Buf,
    status: u8,
}

pub struct Disk {
    // DMA descriptorsの集合。
    // ドライバーがデバイスにどこを読んでどこに書くかを知らせる。
    pub desc: *mut [VirtqDesc; NUM],

    // ドライバーがデバイスに処理してほしいdescriptor番号を書き込むリング
    // descriptor chainの最初の要素のみを保持する
    pub avail: *mut VirtqAvail,

    // デバイスが、処理し終わったchain番号を書くring
    pub used: *mut VirtqUsed,

    pub free: [bool; NUM], // descriptorがfreeか
    pub used_idx: u16,     // used ringを前回見た時どこまで処理されていたか

    // 実行中の操作に関する情報
    // chainの最初のdescriptorのindexに保存される
    pub info: [DiskInfo; NUM],

    // disk commandヘッダ
    // 各descriptorについて保存される
    pub ops: [VirtioBlkReq; NUM],
}

fn reg(r: usize) -> *mut u32 {
    (VIRTIO0 + r) as *mut u32
}

impl Disk {
    pub const fn new() -> Self {
        Self {
            desc: null_mut(),
            avail: null_mut(),
            used: null_mut(),
            free: [false; NUM],
            used_idx: 0,
            info: [const {
                DiskInfo {
                    b: null_mut(),
                    status: 0,
                }
            }; NUM],
            ops: [const { VirtioBlkReq::new() }; NUM],
        }
    }
    pub fn init(&mut self) {
        unsafe {
            assert!(
                reg(VIRTIO_MMIO_MAGIC_VALUE).read_volatile() == 0x74726976
                    && reg(VIRTIO_MMIO_VERSION).read_volatile() == 2
                    && reg(VIRTIO_MMIO_DEVICE_ID).read_volatile() == 2
                    && reg(VIRTIO_MMIO_VENDOR_ID).read_volatile() == 0x554d4551,
                "could not find virtio disk"
            );

            let mut status: u32 = 0;

            // reset device
            reg(VIRTIO_MMIO_STATUS).write_volatile(status);

            // set ACKNOWLEDGE status bit
            status |= VIRTIO_CONFIG_S_ACKNOWLEDGE;
            reg(VIRTIO_MMIO_STATUS).write_volatile(status);

            // set DRIVER status bit
            status |= VIRTIO_CONFIG_S_DRIVER;
            reg(VIRTIO_MMIO_STATUS).write_volatile(status);

            // featuresをネゴる
            let mut features: u32 = reg(VIRTIO_MMIO_DEVICE_FEATURES).read_volatile();
            features &= !(1 << VIRTIO_BLK_F_RO);
            features &= !(1 << VIRTIO_BLK_F_SCSI);
            features &= !(1 << VIRTIO_BLK_F_FLUSH);
            features &= !(1 << VIRTIO_BLK_F_CONFIG_WCE);
            features &= !(1 << VIRTIO_BLK_F_MQ);
            features &= !(1 << VIRTIO_F_ANY_LAYOUT);
            features &= !(1 << VIRTIO_RING_F_EVENT_IDX);
            features &= !(1 << VIRTIO_RING_F_INDIRECT_DESC);
            reg(VIRTIO_MMIO_DRIVER_FEATURES).write_volatile(features);

            // featureネゴシエーションが終わったとデバイスに伝える
            status |= VIRTIO_CONFIG_S_FEATURES_OK;
            reg(VIRTIO_MMIO_STATUS).write_volatile(status);

            // statusを再度読み、FEATURES_OKがセットされているか確認
            status = reg(VIRTIO_MMIO_STATUS).read_volatile();
            assert!(
                (status & VIRTIO_CONFIG_S_FEATURES_OK) != 0,
                "virtio disk FEATURES_OK unset"
            );

            // queue 0を初期化すると伝える
            reg(VIRTIO_MMIO_QUEUE_SEL).write_volatile(0);

            // queue 0が使われていないと保証
            assert!(
                reg(VIRTIO_MMIO_QUEUE_READY).read_volatile() == 0,
                "virtio disk should not be ready"
            );

            // queueの最大サイズを確認
            let max = reg(VIRTIO_MMIO_QUEUE_NUM_MAX).read_volatile();
            assert!(max != 0, "virtio disk has no queue 0");
            assert!(max >= NUM as u32, "virtio disk max queue too short");

            // queueにメモリを割り当て0埋め
            self.desc = kalloc().expect("virtio init").as_ptr() as *mut [VirtqDesc; NUM];
            self.avail = kalloc().expect("virtio init").as_ptr() as *mut VirtqAvail;
            self.used = kalloc().expect("virtio init").as_ptr() as *mut VirtqUsed;
            core::ptr::write_bytes(self.desc as *mut u8, 0, PG_SIZE);
            core::ptr::write_bytes(self.avail as *mut u8, 0, PG_SIZE);
            core::ptr::write_bytes(self.used as *mut u8, 0, PG_SIZE);

            reg(VIRTIO_MMIO_QUEUE_NUM).write_volatile(NUM as u32);

            reg(VIRTIO_MMIO_QUEUE_DESC_LOW).write_volatile(self.desc as *mut () as u32);
            reg(VIRTIO_MMIO_QUEUE_DESC_HIGH)
                .write_volatile((self.desc as *mut () as u64 >> 32) as u32);
            reg(VIRTIO_MMIO_DRIVER_DESC_LOW).write_volatile(self.avail as *mut () as u32);
            reg(VIRTIO_MMIO_DRIVER_DESC_HIGH)
                .write_volatile((self.avail as *mut () as u64 >> 32) as u32);
            reg(VIRTIO_MMIO_DEVICE_DESC_LOW).write_volatile(self.used as *mut () as u32);
            reg(VIRTIO_MMIO_DEVICE_DESC_HIGH)
                .write_volatile((self.used as *mut () as u64 >> 32) as u32);

            // queueは準備完了
            reg(VIRTIO_MMIO_QUEUE_READY).write_volatile(0x1);
            self.free.fill(true);

            // deviceに準備完了したと伝える
            status |= VIRTIO_CONFIG_S_DRIVER_OK;
            reg(VIRTIO_MMIO_STATUS).write_volatile(status);

            // plic.cとtrap.cが残りの設定を行う
        }
    }

    /// freeなdescriptorを割り当てる
    pub fn alloc_desc(&mut self) -> Option<usize> {
        self.free.iter_mut().enumerate().find_map(|(i, free)| {
            if *free {
                *free = false;
                Some(i)
            } else {
                None
            }
        })
    }

    pub fn free_desc(&mut self, i: usize) {
        assert!(i < NUM, "free_desc 1");
        assert!(!self.free[i], "free_desc 2");
        unsafe {
            (*self.desc)[i].addr = 0;
            (*self.desc)[i].len = 0;
            (*self.desc)[i].flags = 0;
            (*self.desc)[i].next = 0;
            self.free[i] = true;
            wakeup(&raw const self.free[0] as *const ());
        }
    }

    pub fn free_chain(&mut self, mut i: usize) {
        loop {
            unsafe {
                let flag = (*self.desc)[i].flags;
                let nxt = (*self.desc)[i].next;
                self.free_desc(i);
                if flag & VRING_DESC_F_NEXT != 0 {
                    i = nxt as usize;
                } else {
                    break;
                }
            }
        }
    }

    /// 3つのdescriptorを割り当てる
    pub fn alloc3_desc(&mut self) -> Option<[usize; 3]> {
        let mut idx = [0; 3];
        for i in 0..3 {
            let Some(v) = self.alloc_desc() else {
                for &j in &idx[..i] {
                    self.free_desc(j);
                }
                return None;
            };
            idx[i] = v;
        }
        Some(idx)
    }
}

pub static DISK_LOCK: SpinLock<Disk> = SpinLock::new(Disk::new(), "virtio_disk");

pub fn virtio_disk_rw(b: *mut Buf, data: &mut BufGuard, write: bool) {
    let mut disk = DISK_LOCK.acquire();

    // ブロックへの操作は、3つのdescriptorが必要
    // type/reserved/sector, data, 1-byte status

    // 3つのdescriptorを取得する
    let idx = loop {
        match disk.alloc3_desc() {
            None => {
                sleep_prepare((&raw const disk.free[0]).cast());
                drop(disk);
                sleep();
                disk = DISK_LOCK.acquire();
            }
            Some(v) => break v,
        }
    };

    // 3つのdescriptorを整形する
    let mut buf0: &mut VirtioBlkReq = &mut disk.ops[idx[0]];

    buf0.mode = if write {
        VIRTIO_BLK_T_OUT
    } else {
        VIRTIO_BLK_T_IN
    };
    buf0.reserved = 0;
    unsafe {
        buf0.sector = (*b).blockno as u64 * (BLOCK_SIZE as u64 / 512);
    }

    unsafe {
        (*disk.desc)[idx[0]].addr = core::ptr::from_mut(buf0) as usize;
        (*disk.desc)[idx[0]].len = size_of::<VirtioBlkReq>() as u32;
        (*disk.desc)[idx[0]].flags = VRING_DESC_F_NEXT;
        (*disk.desc)[idx[0]].next = idx[1] as u16;

        (*disk.desc)[idx[1]].addr = &raw mut data.data as usize;
        (*disk.desc)[idx[1]].len = BLOCK_SIZE;
        (*disk.desc)[idx[1]].flags = if write { 0 } else { VRING_DESC_F_WRITE };
        (*disk.desc)[idx[1]].flags |= VRING_DESC_F_NEXT;
        (*disk.desc)[idx[1]].next = idx[2] as u16;

        disk.info[idx[0]].status = 0xff; // デバイスが書き込みに成功したら0とする
        (*disk.desc)[idx[2]].addr = &raw mut disk.info[idx[0]].status as usize;
        (*disk.desc)[idx[2]].len = 1;
        (*disk.desc)[idx[2]].flags = VRING_DESC_F_WRITE; // statusはデバイスが書く
        (*disk.desc)[idx[2]].next = 0;

        // virtio_disk_intrのためにbも保存する
        (*b).disk = true;
        disk.info[idx[0]].b = b;

        // デバイスにchainの最初を通知可能にする
        let avail_idx = (*disk.avail).idx as usize % NUM;
        (*disk.avail).ring[avail_idx] = idx[0] as u16;

        io_fence();

        // デバイスにring entryが使用可能になったと知らせる
        (*disk.avail).idx = (*disk.avail).idx.wrapping_add(1);

        io_fence();

        reg(VIRTIO_MMIO_QUEUE_NOTIFY).write_volatile(0); // queue numberを入れる
    }

    // virtio_disk_intrがリクエスト完了を報告するまで待つ
    unsafe {
        while (*b).disk {
            sleep_prepare(b.cast());
            drop(disk);
            sleep();
            disk = DISK_LOCK.acquire();
        }
    }

    disk.info[idx[0]].b = null_mut();
    disk.free_chain(idx[0]);
}

pub fn virtio_disk_intr() {
    let mut disk = DISK_LOCK.acquire();

    // デバイスはドライバが割り込みを確認したと通知しない限り別の割り込みを作らない
    // ここで処理通知をするので、デバイスはこの割り込み中にusedリングを進める可能性がある
    // 次の割り込みでは処理することがないかもしれない
    unsafe {
        reg(VIRTIO_MMIO_INTERRUPT_ACK)
            .write_volatile(reg(VIRTIO_MMIO_INTERRUPT_STATUS).read_volatile() & 0x3);

        io_fence();

        // デバイスがdisk.used->idxを加算させる
        while disk.used_idx != (*disk.used).idx {
            io_fence();
            let id = (*disk.used).ring[disk.used_idx as usize % NUM].id;

            assert!(
                disk.info[id as usize].status == 0,
                "virtio_disk_intr status"
            );

            let b: *mut Buf = disk.info[id as usize].b;
            (*b).disk = false;
            wakeup(b.cast());
            disk.used_idx = disk.used_idx.wrapping_add(1);
        }
    }
}
