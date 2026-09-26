// メモリレイアウト

// 00001000 -- boot ROM, provided by qemu
// 02000000 -- CLINT
// 0C000000 -- PLIC
// 10000000 -- uart0
// 10001000 -- virtio disk
// 80000000 -- qemu's boot ROM loads the kernel here,
//             then jumps here.
// unused RAM after 80000000.

pub(crate) const UART0: usize = 0x10000000;
pub(crate) const UART0_IRQ: usize = 10;
