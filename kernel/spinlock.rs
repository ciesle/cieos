use crate::{printk::*, proc::*, riscv::*, uart::*};
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
};

pub trait Lock {
    fn release(&self);
}
pub struct Guard<'a, T, L: Lock> {
    pub lock: &'a L,
    pub data: *mut T,
}
impl<'a, T, L: Lock> Guard<'a, T, L> {
    pub fn new(lock: &'a L, data: *mut T) -> Self {
        Self {
            lock: lock,
            data: data,
        }
    }
}
impl<T, L: Lock> Deref for Guard<'_, T, L> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}
impl<T, L: Lock> DerefMut for Guard<'_, T, L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data }
    }
}
impl<T, L: Lock> Drop for Guard<'_, T, L> {
    fn drop(&mut self) {
        self.lock.release();
    }
}
pub struct SpinLock<T> {
    locked: AtomicBool,
    name: &'static str,
    cpu: UnsafeCell<*const Cpu>,
    data: UnsafeCell<T>,
}
impl<T> SpinLock<T> {
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            locked: AtomicBool::new(false),
            name: name,
            cpu: UnsafeCell::new(null_mut()),
            data: UnsafeCell::new(data),
        }
    }
    pub fn holding(&self) -> bool {
        unsafe { self.locked.load(Ordering::Acquire) && *self.cpu.get() == my_cpu() }
    }
    pub fn acquire(&self) -> Guard<'_, T, SpinLock<T>> {
        push_off(); // deadlockをさけるため、割り込みを停止
                    // ロックを確保した後、割り込みで同じロックを取得しようとすると、デッドロック
        if self.holding() {
            panic!("acquire");
        }

        // atomicなexchangeを行い、ステータスチェック
        // Acquireとすることで、Releaseよりあとに実行されることを保証する
        while self.locked.swap(true, Ordering::Acquire) != false {}

        unsafe {
            *self.cpu.get() = my_cpu();
        }
        Guard::new(self, self.data.get())
    }
    pub unsafe fn acquire_without_lock(&self) -> *const T {
        self.data.get()
    }
    pub unsafe fn reacquire(&self) -> Guard<'_, T, SpinLock<T>> {
        if !self.holding() {
            panic!("reacquire")
        }
        Guard::new(self, self.data.get())
    }
    pub fn release(&self) {
        if !self.holding() {
            panic!("release");
        }
        unsafe {
            *self.cpu.get() = core::ptr::null_mut();
        }
        self.locked.swap(false, Ordering::Release);
        pop_off();
    }
}
impl<T> Lock for SpinLock<T> {
    fn release(&self) {
        SpinLock::release(self);
    }
}
unsafe impl<T> Sync for SpinLock<T> {}

// ロックをかけるとき、割り込みを禁止にする
pub fn push_off() {
    let flags: usize = rc_sstatus(SSTATUS_SIE);
    let old: bool = (flags & SSTATUS_SIE) != 0;

    unsafe {
        if (*my_cpu()).noff == 0 {
            (*my_cpu()).intena = old;
        }
        (*my_cpu()).noff += 1;
    }
}
pub fn pop_off() {
    unsafe {
        let cpu: *mut Cpu = my_cpu();
        if intr_get() {
            panic!("pop_off - interruptible");
        }
        if (*cpu).noff < 1 {
            panic!("pop_off");
        }
        (*cpu).noff -= 1;
        if (*cpu).noff == 0 && (*cpu).intena {
            intr_on();
        }
    }
}
