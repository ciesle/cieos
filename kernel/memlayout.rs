// docs/memlayout.md内に詳細な説明あり
use crate::riscv::*;

pub const UART0: usize = 0x10_000_000;
pub const UART0_IRQ: usize = 10;

pub const VIRTIO0: usize = 0x10_001_000;
pub const VIRTIO0_IRQ: usize = 1;

// core-local interrupt controller
pub const CLINT_BASE: usize = 0x02_000_000;
pub fn clint(hart: u8) -> usize {
    CLINT_BASE + (hart * 4) as usize
}

// platform-level interrupt controller
pub const PLIC: usize = 0x0c_000_000;
pub const PLIC_PRIORITY: usize = PLIC + 0x0;
pub const PLIC_PENDING: usize = PLIC + 0x1000;
pub fn plic_senable(hart: u8) -> usize {
    PLIC + 0x2080 + hart as usize * 0x100
}
pub fn plic_spriority(hart: u8) -> usize {
    PLIC + 0x201_000 + hart as usize * 0x2000
}
pub fn plic_sclaim(hart: u8) -> usize {
    PLIC + 0x201_004 + hart as usize * 0x2000
}

// KERNBASEからPHYSTOPまでをページとして扱う
pub const KERNBASE: usize = 0x80_000_000;
pub const PHYSTOP: usize = KERNBASE + 128 * 1024 * 1024;

// trampoline pageをuserとkernelのどちらにも入れる
pub const TRAMPOLINE: usize = MAX_VA - PG_SIZE;

pub fn kstack(p: usize) -> usize {
    TRAMPOLINE - (p + 1) * 2 * PG_SIZE
}

pub const TRAPFRAME: usize = TRAMPOLINE - PG_SIZE;
