use alloc::{boxed::Box, collections::VecDeque, sync::Arc, vec, vec::Vec};
use spin::{Mutex, RwLock};
use lazy_static::lazy_static;
use x86_64::instructions::interrupts;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::println;

pub mod context;
pub mod sync;

use context::TaskContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug)]
pub struct Task {
    id: usize,
    state: TaskState,
    priority: TaskPriority,
    context: TaskContext,
    stack: Box<[u8]>,
    tls: Option<Box<[u8]>>,
    quantum: usize,
    time_slice: AtomicUsize,
}

impl Task {
    const STACK_SIZE: usize = 4096 * 5; // 20KB stack
    const TLS_SIZE: usize = 4096;       // 4KB TLS
    const DEFAULT_QUANTUM: usize = 100;  // Default time quantum

    pub fn new(entry_point: fn()) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        
        let stack = Box::new([0; Self::STACK_SIZE]);
        let stack_top = stack.as_ptr() as usize + Self::STACK_SIZE;
        
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

        Self {
            id,
            state: TaskState::Ready,
            priority: TaskPriority::Normal,
            context: TaskContext::new(entry_point as usize, stack_top),
            stack,
            tls: Some(Box::new([0; Self::TLS_SIZE])),
            quantum: Self::DEFAULT_QUANTUM,
            time_slice: AtomicUsize::new(Self::DEFAULT_QUANTUM),
        }
    }

    pub fn with_priority(entry_point: fn(), priority: TaskPriority) -> Self {
        let mut task = Self::new(entry_point);
        task.priority = priority;
        task
    }

    pub fn get_tls(&self) -> Option<&[u8]> {
        self.tls.as_ref().map(|tls| tls.as_ref())
    }

    pub fn get_tls_mut(&mut self) -> Option<&mut [u8]> {
        self.tls.as_mut().map(|tls| tls.as_mut())
    }

    pub fn reset_time_slice(&self) {
        self.time_slice.store(self.quantum, Ordering::SeqCst);
    }

    pub fn decrement_time_slice(&self) -> bool {
        self.time_slice.fetch_sub(1, Ordering::SeqCst) <= 1
    }
}

pub struct Scheduler {
    tasks: Vec<VecDeque<Arc<RwLock<Task>>>>,
    current: Option<Arc<RwLock<Task>>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: vec![VecDeque::new(); 3], // One queue per priority level
            current: None,
        }
    }

    pub fn spawn(&mut self, entry_point: fn()) {
        self.spawn_with_priority(entry_point, TaskPriority::Normal);
    }

    pub fn spawn_with_priority(&mut self, entry_point: fn(), priority: TaskPriority) {
        let task = Arc::new(RwLock::new(Task::with_priority(entry_point, priority)));
        self.tasks[priority as usize].push_back(task);
    }

    pub fn schedule(&mut self) -> Option<Arc<RwLock<Task>>> {
        // Check if current task's time slice is expired
        if let Some(ref current) = self.current {
            let task = current.read();
            if !task.decrement_time_slice() {
                return self.current.clone();
            }
        }

        // Move current task back to ready queue if not terminated
        if let Some(current) = self.current.take() {
            let mut task = current.write();
            if task.state != TaskState::Terminated {
                task.state = TaskState::Ready;
                task.reset_time_slice();
                self.tasks[task.priority as usize].push_back(Arc::clone(&current));
            }
        }

        // Find next task to run (highest priority first)
        for priority in (0..self.tasks.len()).rev() {
            if let Some(task) = self.tasks[priority].pop_front() {
                task.write().state = TaskState::Running;
                self.current = Some(task);
                return self.current.clone();
            }
        }

        self.current.clone()
    }

    pub fn block_current(&mut self) {
        if let Some(ref current) = self.current {
            current.write().state = TaskState::Blocked;
        }
        self.schedule();
    }

    pub fn unblock_task(&mut self, task: Arc<RwLock<Task>>) {
        let priority = task.read().priority as usize;
        task.write().state = TaskState::Ready;
        self.tasks[priority].push_back(task);
    }
}

lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

pub fn spawn(entry_point: fn()) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().spawn(entry_point);
    });
}

pub fn spawn_with_priority(entry_point: fn(), priority: TaskPriority) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().spawn_with_priority(entry_point, priority);
    });
}

pub fn yield_now() {
    unsafe {
        context::switch_context();
    }
}

pub fn block_current() {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().block_current();
    });
}

pub fn unblock_task(task: Arc<RwLock<Task>>) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().unblock_task(task);
    });
}

pub fn init() {
    println!("Task scheduler initialized");
} 