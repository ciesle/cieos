#![no_std]
#![no_main]

#[macro_use]
mod printk;
mod console;
mod kalloc;
mod memlayout;
mod param;
mod plic;
mod proc;
mod riscv;
mod sleeplock;
mod spinlock;
mod start;
mod syscall;
mod sysfile;
mod test_iocheck;
mod trampoline;
mod trap;
mod types;
mod uart;
mod vm;
use crate::console::*;
use crate::kalloc::*;
use crate::memlayout::*;
use crate::memlayout::*;
use crate::param::*;
use crate::plic::*;
use crate::printk::*;
use crate::proc::*;
use crate::riscv::*;
use crate::sleeplock::*;
use crate::spinlock::*;
use crate::syscall::*;
use crate::trampoline::*;
use crate::trap::*;
use crate::types::*;
use crate::vm::*;
use core::sync::atomic::{AtomicBool, Ordering};

use core::panic::PanicInfo;
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    unsafe {
        PANICKING.store(true, Ordering::Relaxed);
        printk!("panic: {}\n", info);
        PANICKED.store(true, Ordering::Relaxed);
    }
    loop {}
}

static STARTED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    if cpu_id() == 0 {
        console_init();
        printk!("\ncieos kernel is booting!\n\n");
        kinit();
        KERNEL_PAGETABLE.init();
        KERNEL_PAGETABLE.init_hart();
        proc_init();
        trap_init_hart();
        plic_init();
        plic_init_hart();
        user_init();
        printk!("\nbooting done!\n\n");

        STARTED.store(true, Ordering::Release);
    } else {
        while !STARTED.load(Ordering::Acquire) {}
        printk!("hart {} starting!\n", cpu_id());
        KERNEL_PAGETABLE.init_hart();
        trap_init_hart();
        plic_init_hart();
    }

    scheduler();
}
