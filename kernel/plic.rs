use crate::memlayout::*;
use crate::proc::*;

//
// riscv Platform Level Interrupt Controller
//

// IRQ prioritiesをnon-zeroに変える
pub fn plic_init() {
    unsafe {
        *((PLIC + UART0_IRQ * 4) as *mut u32) = 1;
        *((PLIC + VIRTIO0_IRQ * 4) as *mut u32) = 1;
    }
}

pub fn plic_init_hart() {
    let hart = cpu_id();

    unsafe {
        // このCPUがSモードでもPLIC割り込みをして良いと設定
        *(plic_senable(hart) as *mut u32) = (1 << UART0_IRQ) | (1 << VIRTIO0_IRQ);

        // S-mode priority thresholdを0に設定
        *(plic_spriority(hart) as *mut u32) = 0;
    }
}

// PLICにどんな割り込みが来たかを聞く
pub fn plic_claim() -> u32 {
    unsafe { *(plic_sclaim(cpu_id()) as *mut u32) }
}

// plicに処理が終わったと伝える
pub fn plic_complete(irq: u32) {
    unsafe { *(plic_sclaim(cpu_id()) as *mut u32) = irq };
}
