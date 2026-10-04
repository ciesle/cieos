use crate::kalloc::*;
use crate::memlayout::*;
use crate::printk::*;
use crate::proc::*;
use crate::riscv::*;
use crate::trampoline::*;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut, Index, IndexMut};
use core::ptr::{null, null_mut, NonNull};

pub type Pte = usize;
pub struct PageTable {
    pub root: *mut [Pte; 512],
}
impl PageTable {
    pub const fn new() -> Self {
        Self { root: null_mut() }
    }
    pub fn init(&mut self) -> bool {
        match kalloc().map(NonNull::as_ptr) {
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

    // vaから始まり、paにアクセスするPTEを作る。
    // kallocされたアドレスがpaとして渡されるのを想定
    // vaとsizeはpage-alignedである必要がある。
    // 成功したらtrueを返す
    pub fn map(&self, va: usize, size: usize, mut pa: usize, perm: usize) -> bool {
        if va % PG_SIZE != 0 {
            panic!("map: va not aligned");
        }
        if size % PG_SIZE != 0 {
            panic!("map: size not aligned");
        }
        if size == 0 {
            panic!("map: size");
        }

        let mut a: usize = va;
        let mut last: usize = va + size - PG_SIZE;
        let mut pte: *mut Pte = null_mut();

        loop {
            if {
                match self.walk(a, true).map(NonNull::as_ptr) {
                    Some(v) => {
                        pte = v;
                        true
                    }
                    None => false,
                }
            } {
                unsafe {
                    if *pte & PTE_V != 0 {
                        // 同じページを2度割り当てようとした
                        panic!("map: remap");
                    }
                    *pte = pa2pte(pa) | perm | PTE_V;
                    if a == last {
                        return true;
                    }
                    a += PG_SIZE;
                    pa = pa + PG_SIZE;
                }
            } else {
                return false;
            }
        }
    }

    // vaから始まるnpagesページの割当を外す
    // do_freeがtrueなら、kfreeもしておく
    pub fn unmap(&self, va: usize, npages: usize, do_free: bool) {
        if va % PG_SIZE != 0 {
            panic!("unmap: not aligned");
        }
        for a in (va..(va + npages * PG_SIZE)).step_by(PG_SIZE) {
            match self.walk(a, false).map(NonNull::as_ptr) {
                None => {
                    continue;
                }
                Some(pte) => unsafe {
                    if *pte & PTE_V == 0 {
                        continue;
                    }
                    if do_free {
                        kfree(NonNull::new_unchecked(pte2pa(*pte) as *mut u8));
                    }
                    *pte = 0;
                },
            }
        }

        // 親プロセスのページテーブルを渡されたら、メモリを子のページテーブルにコピーする
    }

    // 仮想メモリサイズをoldszからnewszに増やす
    // 物理メモリの確保と仮想メモリへの割当をどちらも行う高レベルラッパ
    pub fn alloc(&self, oldsz: usize, newsz: usize, xperm: usize) -> usize {
        if newsz < oldsz {
            return oldsz;
        }
        for a in (pgroundup(oldsz)..newsz).step_by(PG_SIZE) {
            match kalloc() {
                Some(mem) => {
                    unsafe {
                        core::ptr::write_bytes(mem.as_ptr(), 0, PG_SIZE);
                    }
                    if !self.map(a, PG_SIZE, mem.as_ptr() as usize, PTE_R | PTE_U | xperm) {
                        kfree(mem);
                        self.dealloc(a, oldsz);
                        return 0;
                    }
                }
                None => {
                    self.dealloc(a, oldsz);
                    return 0;
                }
            }
        }
        newsz
    }

    // user processに割り当てられた仮想メモリ割当を減らす
    pub fn dealloc(&self, oldsz: usize, newsz: usize) -> usize {
        if newsz >= oldsz {
            return oldsz;
        }
        if pgroundup(newsz) < pgroundup(oldsz) {
            let npages = (pgroundup(oldsz) - pgroundup(newsz)) / PG_SIZE;
            self.unmap(pgroundup(newsz), npages, true);
        }
        newsz
    }

    // vaに二対応するPTEのアドレスを返す。
    // allocがfalseなら、必要なpage-tableを作らない
    // 39...63: zero
    // 30...38: 9bitのlevel-2 index
    // 21...29: 9bitのlevel-1 index
    // 12...20: 9bitのlevel-0 index
    //  0...11: 12bitのページ内オフセット
    pub fn walk(&self, va: usize, alloc: bool) -> Option<NonNull<Pte>> {
        if va >= MAX_VA {
            panic!("walk");
        }

        let mut pagetable = self.root;

        for level in (1..=2).rev() {
            unsafe {
                let pte: *mut Pte = &raw mut (*pagetable)[px(level, va)];
                if *pte & PTE_V != 0 {
                    pagetable = pte2pa(*pte) as *mut [Pte; 512];
                } else {
                    if !alloc {
                        return None;
                    }
                    pagetable = match kalloc() {
                        Some(v) => v.as_ptr() as *mut [Pte; 512],
                        None => return None,
                    };
                    *pagetable = [0; 512];
                    *pte = pa2pte(pagetable as usize) | PTE_V;
                }
            }
        }

        unsafe { Some(NonNull::from_ref(&(*pagetable)[px(0, va)])) }
    }
    // 仮想アドレスから物理アドレスを返す
    // user pageを探すためにしか使えない
    fn walk_addr(&self, va: usize) -> Option<usize> {
        if va >= MAX_VA {
            return None;
        }
        unsafe {
            match self.walk(va, false).map(NonNull::as_ptr) {
                Some(v) => {
                    if *v & PTE_V == 0 || *v & PTE_U == 0 {
                        None
                    } else {
                        Some(pte2pa(*v))
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
                if pte & PTE_V != 0 && pte & (PTE_R | PTE_W | PTE_X) == 0 {
                    Self::freewalk_inner(pte2pa(pte) as *mut [Pte; 512]);
                    (*page)[i] = 0;
                } else if (*page)[i] & PTE_V != 0 {
                    panic!("freewalk: leaf");
                }
            }
            kfree(NonNull::new_unchecked(page as *mut u8));
        }
    }
    // 再帰的にページテーブルを開放していく
    // すべての葉の割当は解除されている必要がある
    fn freewalk(&self) {
        Self::freewalk_inner(self.root);
    }

    // user memory pageとpage-table-pagesをfreeする
    // szを上限としてfree。事前に割り当てられた他のページはunmapされている想定
    pub fn free(&self, sz: usize) {
        if sz > 0 {
            self.unmap(0, pgroundup(sz) / PG_SIZE, true);
        }
        self.freewalk();
    }

    // 親のページテーブルに対して呼び出し、メモリを子のページテーブルに割り当てる
    // ページテーブルと物理メモリをコピーする
    pub fn copy(&self, new: &PageTable, sz: usize) -> bool {
        let result = 'copy: {
            for i in (0..sz).step_by(PG_SIZE) {
                let pte = match self.walk(i, false).map(NonNull::as_ptr) {
                    Some(pte) => pte,
                    None => continue, // page tableが割り当てられていなかった
                };
                unsafe {
                    if *pte & PTE_V == 0 {
                        continue; // 物理ページが割り当てられていなかった
                    }
                    let pa = pte2pa(*pte);
                    let flags = pte_flags(*pte);
                    let mem = match kalloc() {
                        Some(mem) => mem,
                        None => break 'copy Err(i),
                    };
                    core::ptr::copy(pa as *const u8, mem.as_ptr(), PG_SIZE);
                    if !new.map(i, PG_SIZE, mem.as_ptr() as usize, flags) {
                        kfree(mem);
                        break 'copy Err(i);
                    }
                }
            }
            Ok(())
        };
        match result {
            Ok(()) => true,
            Err(i) => {
                new.unmap(0, i / PG_SIZE, true);
                false
            }
        }
    }

    // PTEをuser access invalidに設定する
    // execによりuser stack guard pageについて使われる
    fn clear(&self, va: usize) {
        match self.walk(va, false).map(NonNull::as_ptr) {
            Some(pte) => unsafe {
                *pte &= !PTE_U;
            },
            None => panic!("uvmclear"),
        };
    }

    // もしプロセスがsys_sbrkによりlazily allocatedされたページを参照している場合、
    // allocate & mapする。ページフォールト用
    // psz: プロセスのメモリサイズ、va: 仮想アドレス
    pub fn fault(&self, psz: usize, mut va: usize) -> Option<usize> {
        if va >= psz {
            return None;
        }
        va = pgrounddown(va);
        if self.ismapped(va) {
            return None;
        }

        match kalloc() {
            None => return None,
            Some(mem) => unsafe {
                core::ptr::write_bytes(mem.as_ptr(), 0, PG_SIZE);
                if !self.map(va, PG_SIZE, mem.as_ptr() as usize, PTE_W | PTE_U | PTE_R) {
                    kfree(mem);
                    return None;
                }
                Some(mem.as_ptr() as usize)
            },
        }
    }

    // vaはmapされているか？
    fn ismapped(&self, va: usize) -> bool {
        match self.walk(va, false).map(NonNull::as_ptr) {
            Some(pte) => unsafe {
                if *pte & PTE_V != 0 {
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
            self.map(UART0, UART0, PG_SIZE, PTE_R | PTE_W);
            self.map(VIRTIO0, VIRTIO0, PG_SIZE, PTE_R | PTE_W);
            self.map(PLIC, PLIC, 0x4_000_000, PTE_R | PTE_W);
            self.map(KERNBASE, KERNBASE, ETEXT as usize - KERNBASE, PTE_R | PTE_X);
            self.map(
                ETEXT as usize,
                ETEXT as usize,
                PHYSTOP - ETEXT as usize,
                PTE_R | PTE_W,
            );
            self.map(TRAMPOLINE, TRAMPOLINE_PHY as usize, PG_SIZE, PTE_R | PTE_X);
        }
        proc_mapstacks(&self);
    }
    // kernel page tableにmappingを追加。boot時のみで使われる
    pub fn map(&self, va: usize, pa: usize, sz: usize, perm: usize) {
        if !PageTable::map(self, va, sz, pa, perm) {
            panic!("kvmmap");
        }
    }
    // 現在のCPUのhardware page table registerをkernel page tableのものに切り替える
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

// kernelからuserにコピーする
// srcからこのページテーブルのdstvaにlen byteコピーする。
pub fn copyout(
    pagetable: &PageTable,
    psz: usize,
    mut dstva: usize,
    mut src: usize,
    mut len: usize,
) -> bool {
    while len > 0 {
        let va0 = pgrounddown(dstva);
        if va0 >= MAX_VA {
            return false;
        }
        let mut pa0 = match pagetable.walk_addr(va0) {
            None => match pagetable.fault(psz, va0) {
                Some(pa0) => pa0,
                None => return false,
            },
            Some(v) => v,
        };
        // 書き込み禁止なら落とす
        match pagetable.walk(va0, false).map(NonNull::as_ptr) {
            Some(pte) => unsafe {
                if *pte & PTE_W == 0 {
                    return false;
                }
            },
            None => {}
        };
        let n = core::cmp::min(PG_SIZE - (dstva - va0), len);
        unsafe {
            core::ptr::copy(src as *const u8, (pa0 + (dstva - va0)) as *mut u8, n);
        }
        len -= n;
        src += n;
        dstva = va0 + PG_SIZE;
    }
    true
}

// userからkernelにコピーする
pub fn copyin(
    pagetable: &PageTable,
    psz: usize,
    mut dst: usize,
    mut srcva: usize,
    mut len: usize,
) -> bool {
    while len > 0 {
        let va0 = pgrounddown(srcva);
        let mut pa0 = match pagetable.walk_addr(va0) {
            None => match pagetable.fault(psz, va0) {
                Some(pa0) => pa0,
                None => return false,
            },
            Some(v) => v,
        };
        let n = core::cmp::min(PG_SIZE - (srcva - va0), len);
        unsafe {
            core::ptr::copy((pa0 + (srcva - va0)) as *const u8, dst as *mut u8, n);
        }
        len -= n;
        dst += n;
        srcva = va0 + PG_SIZE;
    }
    true
}

// uerからkernelにnull-terminated stringをコピーする
// srcvaからdstへ'\0'が出るかmaxまでコピー
// 成功でtrue
pub fn copyin_str(
    pagetable: &PageTable,
    psz: usize,
    mut dst: usize,
    mut srcva: usize,
    mut max: usize,
) -> Option<usize> {
    let initial_dst = dst;
    while max > 0 {
        let va0 = pgrounddown(srcva);
        let mut pa0 = match pagetable.walk_addr(va0) {
            None => match pagetable.fault(psz, va0) {
                Some(pa0) => pa0,
                None => return None,
            },
            Some(v) => v,
        };
        let mut n = core::cmp::min(PG_SIZE - (srcva - va0), max);

        let mut p: *const u8 = (pa0 + (srcva - va0)) as *const u8;
        unsafe {
            while n > 0 {
                if *p == b'\0' {
                    *(dst as *mut u8) = b'\0';
                    return Some(dst - initial_dst);
                } else {
                    *(dst as *mut u8) = *p;
                }
                n -= 1;
                max -= 1;
                p = p.add(1);
                dst += 1;
            }
        }
        srcva = va0 + PG_SIZE;
    }
    None
}
