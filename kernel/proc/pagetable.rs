use super::{kalloc::*, proc::*, trampoline::*};
use crate::{fs::inode::*, memlayout::*, riscv::*};
use core::cell::UnsafeCell;
use core::cmp::min;
use core::ops::{Add, Deref, DerefMut, Index, IndexMut, Range};
use core::ptr::{NonNull, null, null_mut};
use api::param::*;
use api::vm::*;


pub type Pte = usize;
pub struct PageTable {
    pub root: *mut [Pte; 512],
}
impl PageTable {
    pub const fn new() -> Self {
        Self { root: null_mut() }
    }
    pub fn init(&mut self) -> bool {
        match kalloc().map(Frame::as_ptr) {
            None => {
                return false;
            }
            Some(v) => {
                self.root = v as *mut [Pte; 512];
                unsafe {
                    core::ptr::write_bytes(self.root as *mut u8, 0, PG_SIZE);
                }
                return true;
            }
        }
    }

    /// vaから始まり、paにアクセスするPTEを作る。
    /// kallocされたアドレスがpaとして渡されるのを想定
    /// vaとsizeはpage-alignedである必要がある。
    /// 成功したらtrueを返す
    pub fn map(&self, vr: ARange<Page>, pr: ARange<Frame>, perm: u32) -> bool {
        if vr.len() != pr.len() {
            return false;
        }
        let mut pte: *mut Pte = null_mut();

        for (page, frame) in vr.into_iter().zip(pr) {
            let Some(mut pte) = self.walk(page.addr(), true).map(NonNull::as_ptr) else {
                return false;
            };
            unsafe {
                assert!(*pte & PteFlag::V as usize == 0, "map: remap");
                *pte = pa2pte(frame.addr()) | perm as usize | PteFlag::V as usize;
            }
        }
        true
    }

    /// vrの範囲のページの割当を外す
    /// do_freeがtrueなら、kfreeもしておく
    pub fn unmap(&self, vr: ARange<Page>, do_free: bool) {
        for page in vr {
            match self.walk(page.addr(), false).map(NonNull::as_ptr) {
                None => {
                    continue;
                }
                Some(pte) => unsafe {
                    if *pte & PteFlag::V as usize == 0 {
                        continue;
                    }
                    if do_free {
                        kfree(Frame::from(pte2pa(*pte)));
                    }
                    *pte = 0;
                },
            }
        }
    }

    /// 仮想メモリサイズをoldszからnewszに増やす
    /// 物理メモリの確保と仮想メモリへの割当をどちらも行う高レベルラッパ
    pub fn alloc(&self, oldsz: usize, newsz: usize, xperm: u32) -> Option<usize> {
        if newsz < oldsz {
            return Some(oldsz);
        }
        let vr = ARange::new(VAddr(oldsz).page_up().into(), VAddr(newsz).page_up().into());
        for page in vr {
            match kalloc() {
                Some(mem) => {
                    unsafe {
                        core::ptr::write_bytes(mem.raw() as *mut u8, 0, PG_SIZE);
                    }
                    if !self.map(
                        ARange::span(page, 1),
                        ARange::span(mem, 1),
                        PteFlag::R as u32 | PteFlag::U as u32 | xperm,
                    ) {
                        kfree(mem);
                        self.dealloc(page.raw(), oldsz);
                        return None;
                    }
                }
                None => {
                    self.dealloc(page.raw(), oldsz);
                    return None;
                }
            }
        }
        Some(newsz)
    }

    /// user processに割り当てられた仮想メモリ割当を減らす
    pub fn dealloc(&self, oldsz: usize, newsz: usize) -> usize {
        if newsz >= oldsz {
            return oldsz;
        }
        let vr = ARange::new(VAddr(newsz).page_up().into(), VAddr(oldsz).page_up().into());
        self.unmap(vr, true);
        newsz
    }

    /// vaに対応するPTEのアドレスを返す。
    /// allocがfalseなら、必要なpage-tableを作らない
    /// 39...63: zero
    /// 30...38: 9bitのlevel-2 index
    /// 21...29: 9bitのlevel-1 index
    /// 12...20: 9bitのlevel-0 index
    ///  0...11: 12bitのページ内オフセット
    pub fn walk(&self, va: VAddr, alloc: bool) -> Option<NonNull<Pte>> {
        assert!(va < MAX_VA, "walk");

        let mut pagetable = self.root;

        for level in (1..=2).rev() {
            unsafe {
                let pte: *mut Pte = &raw mut (*pagetable)[px(level, va)];
                if *pte & PteFlag::V as usize != 0 {
                    pagetable = pte2pa(*pte).raw() as *mut [Pte; 512];
                } else {
                    if !alloc {
                        return None;
                    }
                    pagetable = match kalloc() {
                        Some(v) => v.as_ptr() as *mut [Pte; 512],
                        None => return None,
                    };
                    *pagetable = [0; 512];
                    *pte = pa2pte(PAddr(pagetable as usize)) | PteFlag::V as usize;
                }
            }
        }

        unsafe { NonNull::new(&raw mut (*pagetable)[px(0, va)]) }
    }
    /// 仮想アドレスから物理アドレスを返す
    /// user pageを探すためにしか使えない
    pub fn walk_addr(&self, va: VAddr) -> Option<PAddr> {
        if va >= MAX_VA {
            return None;
        }
        unsafe {
            match self.walk(va, false).map(NonNull::as_ptr) {
                Some(v) => {
                    if *v & PteFlag::V as usize == 0 || *v & PteFlag::U as usize == 0 {
                        None
                    } else {
                        Some(pte2pa(*v) + (va.raw() - va.page_down().raw()))
                    }
                }
                None => None,
            }
        }
    }

    fn freewalk_inner(page: *mut [Pte; 512]) {
        unsafe {
            for i in 0..512 {
                let pte = (*page)[i];
                if pte & PteFlag::V as usize != 0
                    && pte & (PteFlag::R as usize | PteFlag::W as usize | PteFlag::X as usize) == 0
                {
                    Self::freewalk_inner(pte2pa(pte).raw() as *mut [Pte; 512]);
                    (*page)[i] = 0;
                } else if (*page)[i] & PteFlag::V as usize != 0 {
                    panic!("freewalk: leaf");
                }
            }
            kfree(Frame(PAddr(page as usize)));
        }
    }
    /// 再帰的にページテーブルを開放していく
    /// すべての葉の割当は解除されている必要がある
    pub fn freewalk(&self) {
        Self::freewalk_inner(self.root);
    }

    /// user memory pageとpage-table-pagesをfreeする
    /// szを上限としてfree。事前に割り当てられた他のページはunmapされている想定
    pub fn free(&self, sz: usize) {
        if sz > 0 {
            self.unmap(ARange::new(Page(VAddr(0)), Page(VAddr(sz).page_up())), true);
        }
        self.freewalk();
    }
    pub fn free_all(&self, sz: usize) {
        unsafe {
            self.unmap(ARange::span(TRAMPOLINE, 1), false);
            self.unmap(ARange::span(TRAPFRAME, 1), false);
            self.free(sz);
        }
    }

    /// 親のページテーブルに対して呼び出し、メモリを子のページテーブルに割り当てる
    /// ページテーブルと物理メモリをコピーする
    pub fn copy(&self, new: &PageTable, sz: usize) -> bool {
        let result = 'copy: {
            for page in ARange::new(Page(VAddr(0)), Page(VAddr(sz).page_up())) {
                let pte = match self.walk(page.addr(), false).map(NonNull::as_ptr) {
                    Some(pte) => pte,
                    None => continue, // page tableが割り当てられていなかった
                };
                unsafe {
                    if *pte & PteFlag::V as usize == 0 {
                        continue; // 物理ページが割り当てられていなかった
                    }
                    let pa = pte2pa(*pte);
                    let flags = pte_flags(*pte);
                    let mem = match kalloc() {
                        Some(mem) => mem,
                        None => break 'copy Err(page),
                    };
                    core::ptr::copy(pa.raw() as *const u8, mem.addr().as_mut_ptr(), PG_SIZE);
                    if !new.map(ARange::span(page, 1), ARange::span(mem, 1), flags as u32) {
                        kfree(mem);
                        break 'copy Err(page);
                    }
                }
            }
            Ok(())
        };
        match result {
            Ok(()) => true,
            Err(page) => {
                new.unmap(ARange::<Page>::new(Page(VAddr(0)), page), true);
                false
            }
        }
    }

    /// ELFプログラムのセグメントをvaにロードする。
    /// va~va+szはマップされている必要がある
    /// vr.startはページ境界にいないといけない
    pub fn load_seg(
        &mut self,
        vr: ARange<VAddr>,
        guard: &mut InodeGuard,
        offset: u32,
        sz: u32,
    ) -> bool {
        assert!(vr.len() == sz as usize);
        for (i, va) in vr.into_iter().enumerate().step_by(PG_SIZE) {
            let Some(pa) = self.walk_addr(va) else {
                panic!("loadseg: address should exist");
            };
            let n = min(vr.len() - i, PG_SIZE);
            // my_procにまだ新しいページテーブルが登録されていないため、
            // 物理アドレスに直で書き込む
            if guard.read(
                false,
                ARange::span(VAddr(pa.raw()), n),
                offset + i as u32,
                n as u32,
            ) != Some(n as u32)
            {
                return false;
            }
        }
        true
    }

    /// PTEをuser access invalidに設定する
    /// execによりuser stack guard pageについて使われる
    pub fn clear(&self, page: Page) {
        match self.walk(page.addr(), false).map(NonNull::as_ptr) {
            Some(pte) => unsafe {
                *pte &= !(PteFlag::U as usize);
            },
            None => panic!("uvmclear"),
        };
    }

    /// もしプロセスがsys_sbrkによりlazily allocatedされたページを参照している場合、
    /// allocate & mapする。ページフォールト用。物理ページを返す
    /// psz: プロセスのメモリサイズ、va: 仮想アドレス
    pub fn fault(&self, psz: usize, mut va: VAddr) -> Option<Frame> {
        if va >= VAddr(psz) {
            return None;
        }
        va = va.page_down();
        if self.ismapped(va) {
            return None;
        }

        match kalloc() {
            None => return None,
            Some(mem) => unsafe {
                core::ptr::write_bytes(mem.as_mut_ptr(), 0, PG_SIZE);
                if !self.map(
                    ARange::span(Page(va), 1),
                    ARange::span(mem, 1),
                    PteFlag::W as u32 | PteFlag::U as u32 | PteFlag::R as u32,
                ) {
                    kfree(mem);
                    return None;
                }
                Some(mem)
            },
        }
    }

    /// vaはmapされているか？
    fn ismapped(&self, va: VAddr) -> bool {
        match self.walk(va, false).map(NonNull::as_ptr) {
            Some(pte) => unsafe {
                if *pte & PteFlag::V as usize != 0 {
                    true
                } else {
                    false
                }
            },
            None => false,
        }
    }
}
impl Index<usize> for PageTable {
    type Output = Pte;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &(*self.root)[index] }
    }
}
impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut (*self.root)[index] }
    }
}
unsafe impl Sync for PageTable {}

pub struct KernelPageTable {
    pagetable: UnsafeCell<PageTable>,
}
impl Deref for KernelPageTable {
    type Target = PageTable;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.pagetable.get() }
    }
}
impl DerefMut for KernelPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.pagetable.get() }
    }
}
impl KernelPageTable {
    pub const fn new() -> Self {
        Self {
            pagetable: UnsafeCell::new(PageTable::new()),
        }
    }
    pub fn init(&self) {
        unsafe {
            (*self.pagetable.get()).root = kalloc().expect("kalloc").as_ptr() as *mut [Pte; 512];
        }
        unsafe {
            core::ptr::write_bytes(self.root as *mut u8, 0, PG_SIZE);
            self.map(
                ARange::span(Page(VAddr(UART0)), 1),
                ARange::span(Frame(PAddr(UART0)), 1),
                PteFlag::R as u32 | PteFlag::W as u32,
            );
            self.map(
                ARange::span(Page(VAddr(VIRTIO0)), 1),
                ARange::span(Frame(PAddr(VIRTIO0)), 1),
                PteFlag::R as u32 | PteFlag::W as u32,
            );
            self.map(
                ARange::span(Page(VAddr(PLIC)), 0x4_000_000 / PG_SIZE),
                ARange::span(Frame(PAddr(PLIC)), 0x4_000_000 / PG_SIZE),
                PteFlag::R as u32 | PteFlag::W as u32,
            );
            self.map(
                ARange::new(Page(VAddr(KERNBASE)), Page(VAddr(ETEXT()))),
                ARange::new(Frame(PAddr(KERNBASE)), Frame(PAddr(ETEXT()))),
                PteFlag::R as u32 | PteFlag::X as u32,
            );
            self.map(
                ARange::new(Page(VAddr(ETEXT())), Page(VAddr(PHYSTOP))),
                ARange::new(Frame(PAddr(ETEXT())), Frame(PAddr(PHYSTOP))),
                PteFlag::R as u32 | PteFlag::W as u32,
            );
            // TRAMPOLINE_PHYはKERNBASE~ETEXTに含まれているので、二重写像である。
            // 仮想アドレス　→　物理アドレス　の写像は単射ではない
            self.map(
                ARange::span(TRAMPOLINE, 1),
                ARange::span(TRAMPOLINE_PHY(), 1),
                PteFlag::R as u32 | PteFlag::X as u32,
            );
        }
        proc_mapstacks(&self);
    }
    /// kernel page tableにmappingを追加。boot時のみで使われる
    pub fn map(&self, vr: ARange<Page>, pr: ARange<Frame>, perm: u32) {
        if !PageTable::map(self, vr, pr, perm) {
            panic!("kvmmap");
        }
    }
    /// 現在のCPUのhardware page table registerをkernel page tableのものに切り替える
    pub fn init_hart(&self) {
        // page table entryに対する書き込みが終わるのを待つ
        sfence_vma();
        w_satp(make_satp(&self));
        // TLBをflush
        sfence_vma();
    }
}
unsafe impl Sync for KernelPageTable {}

pub static KERNEL_PAGETABLE: KernelPageTable = KernelPageTable::new();
