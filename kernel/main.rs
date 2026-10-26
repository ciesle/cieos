#![no_std]
#![no_main]

mod dev;
mod fs;
mod memlayout;
mod proc;
mod riscv;
mod start;
mod user;
mod utils;

use dev::console::*;
use fs::{buf::*, plic::*, virtio::*};
use proc::{kalloc::*, pagetable::*, proc::*, trap::*};
use utils::printk::{self, *};

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
        binit();
        // iinit, fileinitはlockの初期化のみなのでいらない
        let mut disk = DISK_LOCK.acquire();
        disk.init();
        drop(disk);
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
