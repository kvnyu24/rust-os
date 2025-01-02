use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::{Mutex, MutexGuard};
use alloc::sync::Arc;
use super::{TaskState, SCHEDULER};

pub struct Semaphore {
    count: Mutex<isize>,
    waiters: Mutex<VecDeque<Arc<AtomicBool>>>,
}

impl Semaphore {
    pub fn new(initial: isize) -> Self {
        Self {
            count: Mutex::new(initial),
            waiters: Mutex::new(VecDeque::new()),
        }
    }

    pub fn acquire(&self) {
        let mut count = self.count.lock();
        if *count > 0 {
            *count -= 1;
            return;
        }

        // Create a waiter flag
        let waiter = Arc::new(AtomicBool::new(false));
        self.waiters.lock().push_back(Arc::clone(&waiter));
        drop(count);

        // Wait until we're woken up
        while !waiter.load(Ordering::SeqCst) {
            super::yield_now();
        }
    }

    pub fn release(&self) {
        let mut count = self.count.lock();
        *count += 1;

        if let Some(waiter) = self.waiters.lock().pop_front() {
            waiter.store(true, Ordering::SeqCst);
        }
    }
}

pub struct Mutex<T> {
    inner: spin::Mutex<T>,
    waiters: Mutex<VecDeque<Arc<AtomicBool>>>,
}

impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: spin::Mutex::new(value),
            waiters: Mutex::new(VecDeque::new()),
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        self.inner.try_lock()
    }

    pub fn lock(&self) -> MutexGuard<T> {
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }

            // Create a waiter flag
            let waiter = Arc::new(AtomicBool::new(false));
            self.waiters.lock().push_back(Arc::clone(&waiter));

            // Wait until we're woken up
            while !waiter.load(Ordering::SeqCst) {
                super::yield_now();
            }
        }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        if let Some(waiter) = self.waiters.lock().pop_front() {
            waiter.store(true, Ordering::SeqCst);
        }
    }
}

pub struct Condvar {
    waiters: Mutex<VecDeque<Arc<AtomicBool>>>,
}

impl Condvar {
    pub fn new() -> Self {
        Self {
            waiters: Mutex::new(VecDeque::new()),
        }
    }

    pub fn wait<T>(&self, mutex: &Mutex<T>) {
        let waiter = Arc::new(AtomicBool::new(false));
        self.waiters.lock().push_back(Arc::clone(&waiter));

        // Release the mutex and wait
        unsafe {
            mutex.inner.force_unlock();
        }

        while !waiter.load(Ordering::SeqCst) {
            super::yield_now();
        }

        // Reacquire the mutex
        let _ = mutex.lock();
    }

    pub fn notify_one(&self) {
        if let Some(waiter) = self.waiters.lock().pop_front() {
            waiter.store(true, Ordering::SeqCst);
        }
    }

    pub fn notify_all(&self) {
        let mut waiters = self.waiters.lock();
        while let Some(waiter) = waiters.pop_front() {
            waiter.store(true, Ordering::SeqCst);
        }
    }
}

pub struct RwLock<T> {
    inner: spin::RwLock<T>,
    read_waiters: Mutex<VecDeque<Arc<AtomicBool>>>,
    write_waiters: Mutex<VecDeque<Arc<AtomicBool>>>,
}

impl<T> RwLock<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: spin::RwLock::new(value),
            read_waiters: Mutex::new(VecDeque::new()),
            write_waiters: Mutex::new(VecDeque::new()),
        }
    }

    pub fn read(&self) -> spin::RwLockReadGuard<T> {
        loop {
            if let Some(guard) = self.inner.try_read() {
                return guard;
            }

            let waiter = Arc::new(AtomicBool::new(false));
            self.read_waiters.lock().push_back(Arc::clone(&waiter));

            while !waiter.load(Ordering::SeqCst) {
                super::yield_now();
            }
        }
    }

    pub fn write(&self) -> spin::RwLockWriteGuard<T> {
        loop {
            if let Some(guard) = self.inner.try_write() {
                return guard;
            }

            let waiter = Arc::new(AtomicBool::new(false));
            self.write_waiters.lock().push_back(Arc::clone(&waiter));

            while !waiter.load(Ordering::SeqCst) {
                super::yield_now();
            }
        }
    }
} 