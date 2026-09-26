use core::arch::asm;

pub(crate) fn r_mhartid() -> usize {
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
pub(crate) const MSTATUS_MPP_MASK: usize = 3 << 11;
pub(crate) const MSTATUS_MPP_M: usize = 3 << 11;
pub(crate) const MSTATUS_MPP_S: usize = 1 << 11;
pub(crate) const MSTATUS_MPP_U: usize = 0 << 11;

pub(crate) fn r_mstatus() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mstatus",
            out(reg) x,
        );
    }
    x
}

pub(crate) fn w_mstatus(x: usize) {
    unsafe {
        asm!(
            "csrw mstatus, {}",
            in(reg) x
        );
    }
}

pub(crate) fn w_mepc(x: usize) {
    unsafe {
        asm!(
            "csrw mepc, {}",
            in(reg) x
        );
    }
}

// sstatus(supervisor status)の読み書き
pub(crate) const SSTATUS_SPP: usize = 1 << 8; // 直前の特権モード
pub(crate) const SSTATUS_SIE: usize = 1 << 1; // Sモードで割り込みを受け付けるか
pub(crate) const SSTATUS_SPIE: usize = 1 << 5; // トラップ直前のSIE

pub(crate) fn r_sstatus() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sstatus",
            out(reg) x
        );
    }
    x
}

pub(crate) fn w_sstatus(x: usize) {
    unsafe {
        asm!(
            "csrw sstatus, {}",
            in(reg) x
        );
    }
}

pub(crate) fn s_sstatus(x: usize) {
    unsafe {
        asm!(
            "csrs sstatus, {}",
            in(reg) x
        );
    }
}

pub(crate) fn c_sstatus(x: usize) {
    unsafe {
        asm!(
            "csrc sstatus, {}",
            in(reg) x
        );
    }
}

pub(crate) fn rc_sstatus(x: usize) -> usize {
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
pub(crate) fn r_sip() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sip",
            out(reg) x
        );
    }
    x
}

pub(crate) fn w_sip(x: usize) {
    unsafe {
        asm!(
            "csrw sip, {}",
            in(reg) x
        );
    }
}

// 特権モードで割り込みを許可するか
pub(crate) const SIE_SEIE: usize = 1 << 9; // external
pub(crate) const SIE_STIE: usize = 1 << 5; // timer

pub(crate) fn r_sie() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sie",
            out(reg) x
        );
    }
    x
}

pub(crate) fn w_sie(x: usize) {
    unsafe {
        asm!(
            "csrw sie, {}",
            in(reg) x
        );
    }
}

// マシンモードで割り込みを許可するか
pub(crate) const MIE_STIE: usize = 1 << 5;

pub(crate) fn r_mie() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mie",
            out(reg) x
        );
    }
    x
}

pub(crate) fn w_mie(x: usize) {
    unsafe {
        asm!(
            "csrw mie, {}",
            in(reg) x
        );
    }
}

// トラップ直前の実行アドレス
pub(crate) fn r_sepc() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, sepc",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_sepc(x: usize) {
    unsafe {
        asm!(
            "csrw sepc, {}",
            in(reg) x
        );
    }
}

// 例外発生時のマシンモードからの移譲
pub(crate) fn r_medeleg() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, medeleg",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_medeleg(x: usize) {
    unsafe {
        asm!(
            "csrw medeleg, {}",
            in(reg) x
        );
    }
}

// 割り込み発生時のマシンモードからの移譲
pub(crate) fn r_mideleg() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, mideleg",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_mideleg(x: usize) {
    unsafe {
        asm!(
            "csrw mideleg, {}",
            in(reg) x
        );
    }
}

// 割り込みベクトルのベースアドレス
// 下位２ビットがモードで、ベクターモードのときは base + 4 * cause のアドレスにジャンプする
pub(crate) fn r_stvec() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, stvec",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_stvec(x: usize) {
    unsafe {
        asm!(
            "csrw stvec, {}",
            in(reg) x
        );
    }
}

// 特権モード向けのタイマー設定（時刻がこの値以上で割り込みが入る）
pub(crate) fn r_stimecmp() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, 0x14d",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_stimecmp(x: usize) {
    unsafe {
        asm!(
            "csrw 0x14d, {}",
            in(reg) x
        );
    }
}

// マシンモードから、下位のモードの実行環境を指定する
pub(crate) const MENVCFG_STCE: usize = 1 << 63; // stimecmpの有効無効
pub(crate) const MENVCFG_ADUE: usize = 1 << 61; // dirty bitのハードウェア書き込みの有効無効
pub(crate) fn r_menvcfg() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "csrr {}, 0x30a",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_menvcfg(x: usize) {
    unsafe {
        asm!(
            "csrw 0x30a, {}",
            in(reg) x
        );
    }
}

// 物理メモリ保護
// 物理メモリについてどう保護するか
pub(crate) fn w_pmpcfg0(x: usize) {
    unsafe {
        asm!(
            "csrw pmpcfg0, {}",
            in(reg) x
        );
    }
}
// 保護対象となる物理アドレス範囲を指定する
pub(crate) fn w_pmpaddr0(x: usize) {
    unsafe {
        asm!(
            "csrw pmpaddr0, {}",
            in(reg) x
        );
    }
}

const SATP_SV39: usize = 8 << 60;
macro_rules! MAKE_SATP {
    ($pagetable: expr) => {
        SATP_SV39 | ($pagetable >> 12)
    };
}

// supervisor address translation and protection
// (仮想記憶方式)(アドレス空間の識別子 for TLB)(ルートページテーブルのページ番号)
pub(crate) fn w_satp(x: usize) {
    unsafe {
        asm!(
            "csrw satp, {}",
            in(reg) x
        );
    }
}
pub(crate) fn r_satp() -> usize {
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
pub(crate) fn r_scause() -> usize {
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
pub(crate) fn r_stval() -> usize {
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
pub(crate) fn w_mcounteren(x: usize) {
    unsafe {
        asm!(
            "csrw mcounteren, {}",
            in(reg) x
        );
    }
}
pub(crate) fn r_mcounteren() -> usize {
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
pub(crate) fn r_time() -> usize {
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
pub(crate) fn intr_on() {
    s_sstatus(SSTATUS_SIE);
}

// 特権モードで割り込みを禁止
pub(crate) fn intr_off() {
    c_sstatus(SSTATUS_SIE);
}

// 特権モードで割り込みは許可されているか
pub(crate) fn intr_get() -> bool {
    (r_sstatus() & SSTATUS_SIE) != 0
}

// spを取得
pub(crate) fn r_sp() -> usize {
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
pub(crate) fn r_tp() -> usize {
    let x: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            out(reg) x
        );
    }
    x
}
pub(crate) fn w_tp(x: usize) {
    unsafe {
        asm!(
            "mv tp, {}",
            in(reg) x
        );
    }
}

// raを取得
pub(crate) fn r_ra() -> usize {
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
pub(crate) fn sfence_vma() {
    unsafe {
        asm!("sfence.vma zero, zero");
    }
}

// fence(MMIOのとき、OoOで順番が壊れないように保証)
pub(crate) fn io_fence() {
    unsafe {
        asm!("fence iorw, iorw");
    }
}

// fence for icache
pub(crate) fn icache_fence() {
    unsafe {
        asm!("fence.i");
    }
}

pub(crate) type PteT = usize;
pub(crate) type PagetableT = *mut [PteT; 512];

pub(crate) const PGSIZE: usize = 4096;
pub(crate) const PGSHIFT: usize = 12;

macro_rules! PGROUNDUP {
    ($sz: expr) => {
        ($sz + PGSIZE - 1) & !(PGSIZE - 1)
    };
}
macro_rules! PGROUNDDOWN {
    ($a: expr) => {
        $a & !(PGSIZE - 1)
    };
}

pub(crate) const PTE_V: PteT = 1 << 0; // valid
pub(crate) const PTE_R: PteT = 1 << 1;
pub(crate) const PTE_W: PteT = 1 << 2;
pub(crate) const PTE_X: PteT = 1 << 3;
pub(crate) const PTE_U: PteT = 1 << 4; // user can aaccess

macro_rules! PA2PTE {
    ($pa: expr) => {
        ($pa >> 12) << 10
    };
}
macro_rules! PTE2PA {
    ($pte: expr) => {
        ($pte >> 10) << 12
    };
}
macro_rules! PTE_FLAGS {
    ($pte: expr) => {
        $pte & 0x3FF
    };
}

// 9bitのページテーブルインデックスをレベルごとに取得
const PXMASK: PteT = 0x1FF;
macro_rules! PXSHIFT {
    ($level: expr) => {
        PGSHIFT + 9 * $level
    };
}
macro_rules! PX {
    ($level: expr, $va: expr) => {
        ($va >> PXSHIFT!($level)) & PXMASK
    };
}

// 仮想アドレスの最大値
// 符号拡張の必要性があり面倒なため、38ビット目を使わない
// そのため、sv39の最大アドレスよりも１ビット小さい
pub(crate) const MAXVA: usize = 1 << (9 + 9 + 9 + 12 - 1);
