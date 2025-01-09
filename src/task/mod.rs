use alloc::{boxed::Box, collections::VecDeque, sync::Arc, vec, vec::Vec, collections::BTreeMap};
use spin::{Mutex, RwLock};
use lazy_static::lazy_static;
use x86_64::instructions::interrupts;
use x86_64::instructions::random::RdRand;
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
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug)]
pub struct TaskStatistics {
    created_at: u64,
    total_runtime: u64,
    context_switches: usize,
    last_scheduled: Option<u64>,
}

impl TaskStatistics {
    fn new() -> Self {
        Self {
            created_at: get_current_time(),
            total_runtime: 0,
            context_switches: 0,
            last_scheduled: None,
        }
    }
}

fn get_current_time() -> u64 {
    // Use CPU cycles as a simple monotonic counter
    use core::arch::x86_64::_rdtsc;
    unsafe { _rdtsc() }
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
    deadline: Option<u64>,
    group_id: Option<usize>,
    stats: TaskStatistics,
    base_priority: TaskPriority,
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
            deadline: None,
            group_id: None,
            stats: TaskStatistics::new(),
            base_priority: TaskPriority::Normal,
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

    pub fn set_deadline(&mut self, deadline: u64) {
        self.deadline = Some(deadline);
    }

    pub fn set_group(&mut self, group_id: usize) {
        self.group_id = Some(group_id);
    }

    pub fn boost_priority(&mut self) {
        if self.priority != TaskPriority::High {
            self.priority = match self.priority {
                TaskPriority::Low => TaskPriority::Normal,
                TaskPriority::Normal => TaskPriority::High,
                TaskPriority::High => TaskPriority::High,
            };
        }
    }

    pub fn reset_priority(&mut self) {
        self.priority = self.base_priority;
    }

    pub fn suspend(&mut self) {
        if self.state != TaskState::Terminated {
            self.state = TaskState::Suspended;
        }
    }

    pub fn resume(&mut self) {
        if self.state == TaskState::Suspended {
            self.state = TaskState::Ready;
        }
    }

    pub fn get_stats(&self) -> &TaskStatistics {
        &self.stats
    }
}

pub struct Scheduler {
    tasks: Vec<VecDeque<Arc<RwLock<Task>>>>,
    current: Option<Arc<RwLock<Task>>>,
    task_groups: BTreeMap<usize, Vec<Arc<RwLock<Task>>>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: vec![VecDeque::new(); 3], // One queue per priority level
            current: None,
            task_groups: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, entry_point: fn()) {
        self.spawn_with_priority(entry_point, TaskPriority::Normal);
    }

    pub fn spawn_with_priority(&mut self, entry_point: fn(), priority: TaskPriority) {
        let task = Arc::new(RwLock::new(Task::with_priority(entry_point, priority)));
        self.tasks[priority as usize].push_back(task);
    }

    pub fn spawn_with_deadline(&mut self, entry_point: fn(), deadline: u64) {
        let mut task = Task::new(entry_point);
        task.set_deadline(deadline);
        let task = Arc::new(RwLock::new(task));
        self.tasks[TaskPriority::Normal as usize].push_back(task);
    }

    pub fn spawn_in_group(&mut self, entry_point: fn(), group_id: usize) {
        let mut task = Task::new(entry_point);
        task.set_group(group_id);
        let task = Arc::new(RwLock::new(task));
        self.task_groups.entry(group_id)
            .or_insert_with(Vec::new)
            .push(Arc::clone(&task));
        self.tasks[TaskPriority::Normal as usize].push_back(task);
    }

    pub fn suspend_group(&mut self, group_id: usize) {
        if let Some(tasks) = self.task_groups.get(&group_id) {
            for task in tasks {
                task.write().suspend();
            }
        }
    }

    pub fn resume_group(&mut self, group_id: usize) {
        if let Some(tasks) = self.task_groups.get(&group_id) {
            for task in tasks {
                task.write().resume();
            }
        }
    }

    pub fn schedule(&mut self) -> Option<Arc<RwLock<Task>>> {
        if let Some(ref current) = self.current {
            let mut task = current.write();
            if let Some(last_scheduled) = task.stats.last_scheduled {
                task.stats.total_runtime += get_current_time() - last_scheduled;
            }
            task.stats.context_switches += 1;
        }

        for priority_queue in &mut self.tasks {
            for task in priority_queue.iter() {
                let mut task = task.write();
                if let Some(deadline) = task.deadline {
                    if get_current_time() > deadline {
                        task.boost_priority();
                    }
                }
            }
        }

        if let Some(ref current) = self.current {
            let task = current.read();
            if !task.decrement_time_slice() {
                return self.current.clone();
            }
        }

        if let Some(current) = self.current.take() {
            let mut task = current.write();
            if task.state != TaskState::Terminated && task.state != TaskState::Suspended {
                task.state = TaskState::Ready;
                task.reset_time_slice();
                self.tasks[task.priority as usize].push_back(Arc::clone(&current));
            }
        }

        for priority in (0..self.tasks.len()).rev() {
            if let Some(task) = self.tasks[priority].pop_front() {
                let mut task_write = task.write();
                task_write.state = TaskState::Running;
                task_write.stats.last_scheduled = Some(get_current_time());
                drop(task_write);
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

pub fn spawn_with_deadline(entry_point: fn(), deadline: u64) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().spawn_with_deadline(entry_point, deadline);
    });
}

pub fn spawn_in_group(entry_point: fn(), group_id: usize) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().spawn_in_group(entry_point, group_id);
    });
}

pub fn suspend_group(group_id: usize) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().suspend_group(group_id);
    });
}

pub fn resume_group(group_id: usize) {
    interrupts::without_interrupts(|| {
        SCHEDULER.lock().resume_group(group_id);
    });
} 