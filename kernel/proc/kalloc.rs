/*
物理メモリのアロケータ
（ユーザプロセス、カーネルスタック、ページテーブルページ、パイプバッファ）
*/
use super::{spinlock::*, trampoline::*};
use crate::memlayout::*;
use core::ptr::null;
use core::ptr::{NonNull, null_mut};
use api::param::*;
use api::vm::*;


struct Run {
    next: *mut Run,
}
static KMEM: SpinLock<*mut Run> = SpinLock::new(null_mut(), "kmem");

pub fn kinit() {
    unsafe {
        free_range(ARange::new(END(), PAddr(PHYSTOP)));
    }
}

fn free_range(range: ARange<PAddr>) {
    let (start, end) = (range.start.page_up(), range.end.page_down());
    let mut nr = ARange::new(Frame::from(start), Frame::from(end));
    for page in nr {
        kfree(page);
    }
}

/// paで指定されたページを開放する
/// paは、基本的にkallocで割り当てられる（kinitのみが例外）
pub fn kfree(pa: Frame) {
    unsafe {
        // ページテーブル以外を拒否、カーネル領域を拒否、物理メモリ上限以上を拒否
        assert!(
            pa.raw() % PG_SIZE == 0 && pa.addr() >= END() && pa.addr() < PAddr(PHYSTOP),
            "kfree"
        );
        core::ptr::write_bytes(pa.as_mut_ptr(), 1, PG_SIZE);
        let r = pa.raw() as *mut Run;

        let mut freelist = KMEM.acquire();
        (*r).next = *freelist;
        *freelist = r;
    }
}

/// 一つの4096byteのページを割り当てる
/// カーネルが使えるアドレスが返し、割り当てられなければ0を返す
pub fn kalloc() -> Option<Frame> {
    unsafe {
        let mut freelist = KMEM.acquire();
        let r = *freelist;
        if r.is_null() {
            return None;
        }
        *freelist = (**freelist).next;
        drop(freelist);
        if r != null_mut() {
            core::ptr::write_bytes(r as *mut u8, 5, PG_SIZE);
        }
        Some(Frame(PAddr(r as usize)))
    }
}
