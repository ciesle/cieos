use crate::memlayout::*;

core::arch::global_asm!(
    include_str!("trampoline.S"),
    TRAPFRAME = const TRAPFRAME,
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
pub const ETEXT: *const u8 = unsafe { &raw const ETEXT_LABEL };
pub const USERVEC: *const u8 = unsafe { &raw const USERVEC_LABEL };
pub const USERRET: *const u8 = unsafe { &raw const USERRET_LABEL };
pub const TRAMPOLINE_PHY: *const u8 = unsafe { &raw const TRAMPOLINE_LABEL };
pub const END: *const u8 = unsafe { &raw const END_LABEL };
