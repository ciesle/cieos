use crate::{proc::*, riscv::*, uart::*};
use core::sync::atomic::{AtomicBool, Ordering};

struct SpinLock {
    locked: AtomicBool,
    name: &'static str,
    cpu: *mut Cpu,
}

fn init_lock(lk: &mut SpinLock, name: &str) {
    lk.name = name;
    lk.locked = AtomicBool::new(false);
    lk.cpu = core::ptr::null_mut();
}

fn acquire(lk: &mut SpinLock) {
    push_off(); // deadlockをさけるため、割り込みを停止
                // ロックを確保した後、割り込みで同じロックを取得しようとすると、デッドロック
    if holding(lk) {
        panic("acquire");
    }

    // atomicなexchangeを行い、ステータスチェック
    // Acquireとすることで、Releaseよりあとに実行されることを保証する
    while lk.locked.swap(true, Ordering::Acquire) != false {}

    lk.cpu = my_cpu();
}

fn release(lk: &mut SpinLock) {
    if !holding(lk) {
        panic("release");
    }

    lk.cpu = core::ptr::null_mut();

    lk.locked.swap(false, Ordering::Release);

    pop_off();
}

fn holding(lk: *const SpinLock) -> bool {
    lk.locked() && lk.cpu == my_cpu()
}

// ロックをかけるとき、割り込みを禁止にする
fn push_off() {
    //
    let flags: usize = rc_sstatus(SSTATUS_SIE);
    let old: bool = (flags & SSTATUS_SIE) != 0;

    if my_cpu().noff == 0 {
        my_cpu().intena = old;
    }
    my_cpu().noff += 1;
}

fn pop_off() {
    let cpu: *mut Cpu = my_cpu();
    if intr_get() {
        panic("pop_off - interruptible");
    }
    if c.noff < 1 {
        panic("pop_off");
    }
    c.noff -= 1;
    if c.noff == 0 && c.intena {
        intr_on();
    }
}
