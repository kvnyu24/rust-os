use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::{Mutex as SpinMutex, MutexGuard};
use alloc::sync::Arc;

pub struct Semaphore {
    count: AtomicUsize,
    waiters: SpinMutex<VecDeque<Arc<AtomicBool>>>,
}

impl Semaphore {
    pub const fn new(initial: usize) -> Self {
        Self {
            count: AtomicUsize::new(initial),
            waiters: SpinMutex::new(VecDeque::with_capacity(0)),
        }
    }

    pub fn acquire(&self) {
        loop {
            let current = self.count.load(Ordering::SeqCst);
            if current > 0 && self.count.compare_exchange(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                break;
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

    pub fn release(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
        if let Some(waiter) = self.waiters.lock().pop_front() {
            waiter.store(true, Ordering::SeqCst);
        }
    }
}

pub struct BlockingMutex<T> {
    inner: SpinMutex<T>,
    waiters: SpinMutex<VecDeque<Arc<AtomicBool>>>,
}

impl<T> BlockingMutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: SpinMutex::new(value),
            waiters: SpinMutex::new(VecDeque::new()),
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

    pub fn unlock_next(&self) {
        if let Some(waiter) = self.waiters.lock().pop_front() {
            waiter.store(true, Ordering::SeqCst);
        }
    }
}

pub struct Condvar {
    waiters: SpinMutex<VecDeque<Arc<AtomicBool>>>,
}

impl Condvar {
    pub const fn new() -> Self {
        Self {
            waiters: SpinMutex::new(VecDeque::new()),
        }
    }

    pub fn wait<T>(&self, mutex: &BlockingMutex<T>) {
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
    read_waiters: SpinMutex<VecDeque<Arc<AtomicBool>>>,
    write_waiters: SpinMutex<VecDeque<Arc<AtomicBool>>>,
}

impl<T> RwLock<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: spin::RwLock::new(value),
            read_waiters: SpinMutex::new(VecDeque::new()),
            write_waiters: SpinMutex::new(VecDeque::new()),
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