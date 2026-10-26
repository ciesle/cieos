#![allow(non_snake_case)]
use crate::memlayout::*;

use api::vm::*;

core::arch::global_asm!(
    include_str!("trampoline.S"),
    TRAPFRAME = const TRAPFRAME.0.0,
);

unsafe extern "C" {
    #[link_name = "etext"]
    static ETEXT_LABEL: u8;
    #[link_name = "uservec"]
    static USERVEC_LABEL: u8;
    #[link_name = "userret"]
    static USERRET_LABEL: u8;
    #[link_name = "trampoline"]
    static TRAMPOLINE_LABEL: u8;
    #[link_name = "end"]
    static END_LABEL: u8;
}
pub fn ETEXT() -> usize {
    unsafe { &raw const ETEXT_LABEL as usize }
}
pub fn USERVEC() -> PAddr {
    unsafe { PAddr(&raw const USERVEC_LABEL as usize) }
}
pub fn USERRET() -> PAddr {
    unsafe { PAddr(&raw const USERRET_LABEL as usize) }
}
pub fn TRAMPOLINE_PHY() -> Frame {
    unsafe { Frame(PAddr(&raw const TRAMPOLINE_LABEL as usize)) }
}
pub fn END() -> PAddr {
    unsafe { PAddr(&raw const END_LABEL as usize) }
}
