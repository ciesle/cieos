use crate::spinlock::*;
use core::sync::atomic::AtomicBool;

struct SleepLock {
    locked: AtomicBool,
    lk: SpinLock, // このsleep lockを保護するspin lock
    name: &str,
    pid: u32,
}

fn inits_sleeplock(lk: &mut sleeplock, name: &str) {
    init_lock(&lk.lk, "sleep lock");
    lk.name = name;
    lk.locked = AtomicBool::new(false);
    lk.pid = 0;
}

fn acquire_sleep(lk: &mut SleepLock) {
    acquire(&lk.lk);
    while lk.locked.load(Order::Acquire) {
        sleep_prepare(lk);
    }
}
