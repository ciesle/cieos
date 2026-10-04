use crate::printk::*;
use crate::proc::*;
use crate::spinlock::*;
use core::cell::UnsafeCell;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

struct SleepLockState {
    locked: bool,
    pid: u32,
}
pub struct SleepLock<T> {
    lk: SpinLock<SleepLockState>, // このsleep lockを保護するspin lock
    name: &'static str,
    data: UnsafeCell<T>,
}
impl<T> SleepLock<T> {
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            lk: SpinLock::new(
                SleepLockState {
                    locked: false,
                    pid: 0,
                },
                "sleeplock",
            ),
            name: name,
            data: UnsafeCell::new(data),
        }
    }
    pub fn acquire(&self) -> Guard<'_, T, SleepLock<T>> {
        let mut state = self.lk.acquire();
        unsafe {
            while state.locked {
                sleep_prepare(core::ptr::from_ref(self).cast::<()>());
                drop(state);
                sleep();
                state = self.lk.acquire();
            }
            state.locked = true;
            state.pid = *(*my_proc()).pid.get();
        }
        Guard::new(self, self.data.get())
    }
    pub fn release(&self) {
        let mut state = self.lk.acquire();
        unsafe {
            state.locked = false;
            state.pid = 0;
            wakeup(core::ptr::from_ref(self).cast::<()>());
        }
    }
    pub fn holding(&self) -> bool {
        let state = self.lk.acquire();
        unsafe { state.locked && (state.pid == *my_proc().pid.get()) }
    }
}
impl<T> Lock for SleepLock<T> {
    fn release(&self) {
        SleepLock::release(self);
    }
}
unsafe impl<T> Sync for SleepLock<T> {}
