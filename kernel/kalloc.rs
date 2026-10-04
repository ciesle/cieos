/*
物理メモリのアロケータ
（ユーザプロセス、カーネルスタック、ページテーブルページ、パイプバッファ）
*/
use crate::memlayout::*;
use crate::printk::*;
use crate::riscv::*;
use crate::spinlock::*;
use crate::trampoline::*;
use crate::vm::*;
use core::ptr::null;
use core::ptr::{null_mut, NonNull};

struct Run {
    next: *mut Run,
}
static KMEM: SpinLock<*mut Run> = SpinLock::new(null_mut(), "kmem");

pub fn kinit() {
    unsafe {
        freerange(END, PHYSTOP as *const u8);
    }
}

fn freerange(pa_start: *const u8, pa_end: *const u8) {
    let mut p = pgroundup(pa_start as usize);
    while p + PG_SIZE <= pa_end as usize {
        kfree(NonNull::new(p as *mut u8).unwrap());
        p += PG_SIZE;
    }
}

// paで指定されたページを開放する
// paは、基本的にkallocで割り当てられる（kinitのみが例外）
pub fn kfree(pa: NonNull<u8>) {
    let addr = pa.as_ptr() as usize;
    unsafe {
        // ページテーブル以外を拒否、カーネル領域を拒否、物理メモリ上限以上を拒否
        if addr % PG_SIZE != 0 || addr < (END as usize) || addr >= PHYSTOP {
            panic!("kfree");
        }
        core::ptr::write_bytes(pa.as_ptr(), 1, PG_SIZE);
        let r = addr as *mut Run;

        let mut freelist = KMEM.acquire();
        (*r).next = *freelist;
        *freelist = r;
    }
}

// 一つの4096byteのページを割り当てる
// カーネルが使えるアドレスが返し、割り当てられなければ0を返す
pub fn kalloc() -> Option<NonNull<u8>> {
    unsafe {
        let mut freelist = KMEM.acquire();
        let r = *freelist;
        if *freelist != null_mut() {
            *freelist = (**freelist).next;
        }
        drop(freelist);
        if r != null_mut() {
            core::ptr::write_bytes(r as *mut u8, 5, PG_SIZE);
        }
        NonNull::new(r as *mut u8)
    }
}
