use core::{future::Future, pin::Pin, task::{Context, Poll}};
use alloc::{boxed::Box, collections::VecDeque, sync::Arc};
use spin::{Mutex, RwLock};
use lazy_static::lazy_static;
use x86_64::instructions::interrupts;

pub mod context;
use context::TaskContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

pub struct Task {
    id: usize,
    state: TaskState,
    context: TaskContext,
    stack: Box<[u8]>,
}

impl Task {
    const STACK_SIZE: usize = 4096 * 5; // 20KB stack

    pub fn new(entry_point: fn()) -> Self {
        let stack = Box::new([0; Self::STACK_SIZE]);
        let stack_top = stack.as_ptr() as usize + Self::STACK_SIZE;
        
        static mut NEXT_ID: usize = 0;
        let id = unsafe {
            let id = NEXT_ID;
            NEXT_ID += 1;
            id
        };

        Self {
            id,
            state: TaskState::Ready,
            context: TaskContext::new(entry_point as usize, stack_top),
            stack,
        }
    }
}

pub struct Scheduler {
    tasks: VecDeque<Arc<RwLock<Task>>>,
    current: Option<Arc<RwLock<Task>>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
            current: None,
        }
    }

    pub fn spawn(&mut self, entry_point: fn()) {
        let task = Arc::new(RwLock::new(Task::new(entry_point)));
        self.tasks.push_back(task);
    }

    pub fn schedule(&mut self) -> Option<Arc<RwLock<Task>>> {
        if let Some(current) = self.current.take() {
            let mut task = current.write();
            if task.state != TaskState::Terminated {
                task.state = TaskState::Ready;
                self.tasks.push_back(Arc::clone(&current));
            }
        }

        self.current = self.tasks.pop_front();
        if let Some(ref task) = self.current {
            task.write().state = TaskState::Running;
        }
        self.current.clone()
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

pub fn yield_now() {
    unsafe {
        context::switch_context();
    }
}

pub fn init() {
    // Initialize any scheduler-related resources
    println!("Task scheduler initialized");
} 