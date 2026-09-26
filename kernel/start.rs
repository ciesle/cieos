use core::arch::asm;

use crate::{param::*, riscv::*};

#[unsafe(link_section = ".stack0")]
#[unsafe(no_mangle)]
static mut stack0: [u8; 4096 * NCPU] = [0; 4096 * NCPU];

#[unsafe(no_mangle)]
fn start() {
    let mut x: usize = r_mstatus();
    x &= !MSTATUS_MPP_MASK;
    x |= MSTATUS_MPP_S;
    w_mstatus(x);

    w_mepc(crate::main as *const () as usize);

    // ページングを切る
    w_satp(0);

    // すべての割り込みを移譲
    w_medeleg(0xffff);
    w_mideleg(0xffff);
    w_sie(r_sie() | SIE_SEIE | SIE_STIE);

    // 物理メモリ保護を設定。すべてのページにアクセス可能
    w_pmpaddr0(0x3fffffffffffff);
    w_pmpcfg0(0xf);

    // ハードウェアのダーティページの書き込みを許可
    w_menvcfg(r_menvcfg() | MENVCFG_ADUE);

    timer_init();

    let id: usize = r_mhartid();
    w_tp(id);

    unsafe {
        asm!("mret");
    }
}

fn timer_init() {
    // 特権モードにタイマー比較を許可
    w_menvcfg(r_menvcfg() | MENVCFG_STCE);

    w_mcounteren(r_mcounteren() | 2);

    w_stimecmp(r_time() + 1000000);
}
