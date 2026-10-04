use crate::vm::*;
use core::arch::asm;

pub fn r_mhartid() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mhartid",
            out(reg) x,
        );
    }
    x
}

// mstatusの読み書き
pub const MSTATUS_MPP_MASK: usize = 3 << 11;
pub const MSTATUS_MPP_M: usize = 3 << 11;
pub const MSTATUS_MPP_S: usize = 1 << 11;
pub const MSTATUS_MPP_U: usize = 0 << 11;

pub fn r_mstatus() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mstatus",
            out(reg) x,
        );
    }
    x
}

pub fn w_mstatus(x: usize) {
    unsafe {
        asm!(
            "csrw mstatus, {}",
            in(reg) x
        );
    }
}

pub fn w_mepc(x: usize) {
    unsafe {
        asm!(
            "csrw mepc, {}",
            in(reg) x
        );
    }
}

// sstatus(supervisor status)の読み書き
pub const SSTATUS_SPP: usize = 1 << 8; // 直前の特権モード
pub const SSTATUS_SIE: usize = 1 << 1; // Sモードで割り込みを受け付けるか
pub const SSTATUS_SPIE: usize = 1 << 5; // トラップ直前のSIE

pub fn r_sstatus() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sstatus",
            out(reg) x
        );
    }
    x
}

pub fn w_sstatus(x: usize) {
    unsafe {
        asm!(
            "csrw sstatus, {}",
            in(reg) x
        );
    }
}

pub fn s_sstatus(x: usize) {
    unsafe {
        asm!(
            "csrs sstatus, {}",
            in(reg) x
        );
    }
}

pub fn c_sstatus(x: usize) {
    unsafe {
        asm!(
            "csrc sstatus, {}",
            in(reg) x
        );
    }
}

pub fn rc_sstatus(x: usize) -> usize {
    let old: usize;
    unsafe {
        asm!(
            "csrrc {old}, sstatus, {x}",
            old = lateout(reg) old,
            x = in(reg) x
        );
    }
    old
}

// 特権モードでの割り込みがpendingか
pub fn r_sip() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sip",
            out(reg) x
        );
    }
    x
}

pub fn w_sip(x: usize) {
    unsafe {
        asm!(
            "csrw sip, {}",
            in(reg) x
        );
    }
}

// 特権モードで割り込みを許可するか
pub const SIE_SEIE: usize = 1 << 9; // external
pub const SIE_STIE: usize = 1 << 5; // timer

pub fn r_sie() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sie",
            out(reg) x
        );
    }
    x
}

pub fn w_sie(x: usize) {
    unsafe {
        asm!(
            "csrw sie, {}",
            in(reg) x
        );
    }
}

// マシンモードで割り込みを許可するか
pub const MIE_STIE: usize = 1 << 5;

pub fn r_mie() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mie",
            out(reg) x
        );
    }
    x
}

pub fn w_mie(x: usize) {
    unsafe {
        asm!(
            "csrw mie, {}",
            in(reg) x
        );
    }
}

// トラップ直前の実行アドレス
pub fn r_sepc() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sepc",
            out(reg) x
        );
    }
    x
}
pub fn w_sepc(x: usize) {
    unsafe {
        asm!(
            "csrw sepc, {}",
            in(reg) x
        );
    }
}

// 例外発生時のマシンモードからの移譲
pub fn r_medeleg() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, medeleg",
            out(reg) x
        );
    }
    x
}
pub fn w_medeleg(x: usize) {
    unsafe {
        asm!(
            "csrw medeleg, {}",
            in(reg) x
        );
    }
}

// 割り込み発生時のマシンモードからの移譲
pub fn r_mideleg() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mideleg",
            out(reg) x
        );
    }
    x
}
pub fn w_mideleg(x: usize) {
    unsafe {
        asm!(
            "csrw mideleg, {}",
            in(reg) x
        );
    }
}

// 割り込みベクトルのベースアドレス
// 下位２ビットがモードで、ベクターモードのときは base + 4 * cause のアドレスにジャンプする
pub fn r_stvec() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, stvec",
            out(reg) x
        );
    }
    x
}
pub fn w_stvec(x: usize) {
    unsafe {
        asm!(
            "csrw stvec, {}",
            in(reg) x
        );
    }
}

// 特権モード向けのタイマー設定（時刻がこの値以上で割り込みが入る）
pub fn r_stimecmp() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, 0x14d",
            out(reg) x
        );
    }
    x
}
pub fn w_stimecmp(x: usize) {
    unsafe {
        asm!(
            "csrw 0x14d, {}",
            in(reg) x
        );
    }
}

// マシンモードから、下位のモードの実行環境を指定する
pub const MENVCFG_STCE: usize = 1 << 63; // stimecmpの有効無効
pub const MENVCFG_ADUE: usize = 1 << 61; // dirty bitのハードウェア書き込みの有効無効
pub fn r_menvcfg() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, 0x30a",
            out(reg) x
        );
    }
    x
}
pub fn w_menvcfg(x: usize) {
    unsafe {
        asm!(
            "csrw 0x30a, {}",
            in(reg) x
        );
    }
}

// 物理メモリ保護
// 物理メモリについてどう保護するか
pub fn w_pmpcfg0(x: usize) {
    unsafe {
        asm!(
            "csrw pmpcfg0, {}",
            in(reg) x
        );
    }
}
// 保護対象となる物理アドレス範囲を指定する
pub fn w_pmpaddr0(x: usize) {
    unsafe {
        asm!(
            "csrw pmpaddr0, {}",
            in(reg) x
        );
    }
}

const SATP_SV39: usize = 8 << 60;
pub fn make_satp(pagetable: &PageTable) -> usize {
    SATP_SV39 | (pagetable.root as usize >> 12)
}

// supervisor address translation and protection
// (仮想記憶方式)(アドレス空間の識別子 for TLB)(ルートページテーブルのページ番号)
pub fn w_satp(x: usize) {
    unsafe {
        asm!(
            "csrw satp, {}",
            in(reg) x
        );
    }
}
pub fn r_satp() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, satp",
            out(reg) x
        );
    }
    x
}

// 特権モードの trap cause
pub fn r_scause() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, scause",
            out(reg) x
        );
    }
    x
}

// 特権モードの trap value
pub fn r_stval() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, stval",
            out(reg) x
        );
    }
    x
}

// マシンモードの下位モードから、各種カウンタを読ませるか
pub fn w_mcounteren(x: usize) {
    unsafe {
        asm!(
            "csrw mcounteren, {}",
            in(reg) x
        );
    }
}
pub fn r_mcounteren() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mcounteren",
            out(reg) x
        );
    }
    x
}

// time カウンタ
pub fn r_time() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, time",
            out(reg) x
        );
    }
    x
}

// 特権モードで割り込みを許可（全体）
pub fn intr_on() {
    s_sstatus(SSTATUS_SIE);
}

// 特権モードでinterrupt trapを禁止
// 割り込みがあった場合、pending bitがたち、放置される。
// intr_on()が呼ばれた後、pendingされていた割り込みがまとめて実行される
pub fn intr_off() {
    c_sstatus(SSTATUS_SIE);
}

// 特権モードで割り込みは許可されているか
pub fn intr_get() -> bool {
    (r_sstatus() & SSTATUS_SIE) != 0
}

// spを取得
pub fn r_sp() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "mv {}, sp",
            out(reg) x
        );
    }
    x
}

// xv6では、tpはスレッドポインタではなく、現在のCPUコアの番号保存のために使う
// cpus[tp]を読むと現在のcpuの状態がわかる
pub fn r_tp() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            out(reg) x
        );
    }
    x
}
pub fn w_tp(x: usize) {
    unsafe {
        asm!(
            "mv tp, {}",
            in(reg) x
        );
    }
}

// raを取得
pub fn r_ra() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "mv {}, ra",
            out(reg) x
        );
    }
    x
}

// TLBをフラッシュ
// PTへの書き込みのfenceにもなっている
pub fn sfence_vma() {
    unsafe {
        asm!("sfence.vma zero, zero");
    }
}

// fence(MMIOのとき、OoOで順番が壊れないように保証)
pub fn io_fence() {
    unsafe {
        asm!("fence iorw, iorw");
    }
}

// fence for icache
pub fn icache_fence() {
    unsafe {
        asm!("fence.i");
    }
}

pub const PG_SIZE: usize = 4096;
pub const PG_SHIFT: usize = 12;

pub fn pgroundup(sz: usize) -> usize {
    (sz + PG_SIZE - 1) & !(PG_SIZE - 1)
}
pub fn pgrounddown(sz: usize) -> usize {
    sz & !(PG_SIZE - 1)
}

pub const PTE_V: Pte = 1 << 0; // valid
pub const PTE_R: Pte = 1 << 1;
pub const PTE_W: Pte = 1 << 2;
pub const PTE_X: Pte = 1 << 3;
pub const PTE_U: Pte = 1 << 4; // user can aaccess

pub fn pa2pte(pa: usize) -> usize {
    (pa >> 12) << 10
}
pub fn pte2pa(pte: usize) -> usize {
    (pte >> 10) << 12
}
pub fn pte_flags(pte: usize) -> usize {
    pte & 0x3FF
}

// 9bitのページテーブルインデックスをレベルごとに取得
const PXMASK: Pte = 0x1FF;
pub fn pxshift(level: usize) -> usize {
    PG_SHIFT + 9 * level
}
pub fn px(level: usize, va: usize) -> usize {
    (va >> pxshift(level)) & PXMASK
}

// 仮想アドレスの最大値
// 符号拡張の必要性があり面倒なため、38ビット目を使わない
// そのため、sv39の最大アドレスよりも１ビット小さい
pub const MAX_VA: usize = 1 << (9 + 9 + 9 + 12 - 1);
