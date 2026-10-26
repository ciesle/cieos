use super::pagetable::*;
use crate::riscv::*;
use core::cell::UnsafeCell;
use core::cmp::min;
use core::ptr::{NonNull, null, null_mut};
use api::param::*;
use api::vm::*;


/// kernelからuserにコピーする
/// srcからこのページテーブルのdstにlen byteコピーする。
/// srcがカーネルなのにcopyoutをpagetableのメンバにすると気持ち悪いので出した
pub fn copyout(pagetable: &PageTable, psz: usize, dst: ARange<VAddr>, src: &[u8]) -> bool {
    assert!(dst.len() == src.len(), "copyout");
    if src.is_empty() {
        return true;
    }
    if dst.start >= MAX_VA {
        return false;
    }
    for (ur, kr) in (dst.start.page_down().raw()..dst.end.raw())
        .step_by(PG_SIZE)
        .map(|base| {
            let start = base.max(dst.start.raw());
            let n = (base + PG_SIZE).min(dst.end.raw()) - start;
            let off = start - dst.start.raw();
            (
                ARange::new(VAddr(start), VAddr(start + n)),
                &src[off..off + n],
            )
        })
    {
        if ur.start >= MAX_VA {
            return false;
        }
        // ur.startに相当する物理アドレスを取得
        let mut pa0 = match pagetable.walk_addr(ur.start) {
            None => match pagetable.fault(psz, ur.start) {
                Some(pa0) => pa0.addr() + (ur.start.raw() % PG_SIZE),
                None => return false,
            },
            Some(v) => v,
        };
        // 書き込み禁止なら落とす
        if pagetable
            .walk(ur.start, false)
            .map(NonNull::as_ptr)
            .is_none_or(|pte| unsafe { *pte & PteFlag::W as usize == 0 })
        {
            return false;
        }
        ARange::new(pa0, pa0 + ur.len()).copy_from_slice(kr);
    }
    true
}

/// userからkernelにコピーする
pub fn copyin(pagetable: &PageTable, psz: usize, dst: &mut [u8], src: ARange<VAddr>) -> bool {
    assert!(dst.len() == src.len(), "copyin");
    if src.len() == 0 {
        return true;
    }
    if src.start >= MAX_VA {
        return false;
    }
    for base in (src.start.page_down().raw()..src.end.raw()).step_by(PG_SIZE) {
        let start = base.max(src.start.raw());
        let n = (base + PG_SIZE).min(src.end.raw()) - start;
        let off = start - src.start.raw();
        let ur = ARange::new(VAddr(start), VAddr(start + n));
        let mut kr = &mut dst[off..off + n];

        // ur.startに相当する物理アドレスを取得
        let mut pa0 = match pagetable.walk_addr(ur.start) {
            None => match pagetable.fault(psz, ur.start) {
                Some(pa0) => pa0.addr() + (ur.start.raw() % PG_SIZE),
                None => return false,
            },
            Some(v) => v,
        };
        kr.copy_from_slice(&ARange::new(pa0, pa0 + ur.len()));
    }
    true
}

/// uerからkernelにnull-terminated stringをコピーする
/// srcvaからdstへ'\0'が出るかmaxまでコピー
/// 成功でtrue
pub fn copyin_str(
    pagetable: &PageTable,
    psz: usize,
    mut dst: VAddr,
    mut src: VAddr,
    mut max: usize,
) -> Option<usize> {
    let initial_dst = dst;
    while max > 0 {
        let va0 = src.page_down();
        let mut pa0 = match pagetable.walk_addr(va0) {
            None => match pagetable.fault(psz, va0) {
                Some(pa0) => pa0.addr(),
                None => return None,
            },
            Some(v) => v,
        };
        let mut n = core::cmp::min(PG_SIZE - (src - va0), max);

        let mut p: PAddr = pa0 + (src - va0);
        unsafe {
            while n > 0 {
                if *p.as_ptr() == b'\0' {
                    *dst.as_mut_ptr() = b'\0';
                    return Some(dst - initial_dst);
                } else {
                    *dst.as_mut_ptr() = *p.as_ptr();
                }
                n -= 1;
                max -= 1;
                p = p + 1;
                dst = dst + 1;
            }
        }
        src = va0 + PG_SIZE;
    }
    None
}
