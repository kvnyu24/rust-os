use alloc::{string::String, vec::Vec, sync::Arc};
use spin::RwLock;
use x86_64::VirtAddr;
use lazy_static::lazy_static;
use crate::{memory, task};

pub mod syscall;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug)]
pub struct Process {
    id: usize,
    state: ProcessState,
    name: String,
    memory_space: memory::MemorySpace,
    task: Arc<RwLock<task::Task>>,
}

impl Process {
    pub fn new(name: String, program: Vec<u8>) -> Result<Self, &'static str> {
        static mut NEXT_PID: usize = 1000;  // PIDs start at 1000 for user processes
        
        let pid = unsafe {
            let pid = NEXT_PID;
            NEXT_PID += 1;
            pid
        };

        // Create a new memory space for the process
        let memory_space = memory::MemorySpace::new()?;
        
        // Load the program into memory
        memory_space.load_program(&program)?;

        // Create a new task for the process
        let entry_point = memory_space.entry_point();
        let task = Arc::new(RwLock::new(task::Task::new(entry_point as fn())));

        Ok(Self {
            id: pid,
            state: ProcessState::Ready,
            name,
            memory_space,
            task,
        })
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn state(&self) -> ProcessState {
        self.state
    }
}

pub struct ProcessManager {
    processes: Vec<Arc<RwLock<Process>>>,
    current: Option<Arc<RwLock<Process>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            current: None,
        }
    }

    pub fn spawn(&mut self, name: String, program: Vec<u8>) -> Result<usize, &'static str> {
        let process = Process::new(name, program)?;
        let pid = process.id();
        let process = Arc::new(RwLock::new(process));
        
        // Add the process's task to the scheduler
        task::spawn(|| {
            // This will be the entry point for the process
            syscall::init_process_context();
            let process = PROCESS_MANAGER.read().current.as_ref().unwrap().clone();
            let entry = process.read().memory_space.entry_point();
            unsafe {
                core::mem::transmute::<usize, fn()>(entry)();
            }
        });

        self.processes.push(Arc::clone(&process));
        Ok(pid)
    }

    pub fn get_process(&self, pid: usize) -> Option<Arc<RwLock<Process>>> {
        self.processes.iter()
            .find(|p| p.read().id() == pid)
            .map(Arc::clone)
    }

    pub fn current_process(&self) -> Option<Arc<RwLock<Process>>> {
        self.current.as_ref().map(Arc::clone)
    }

    pub fn schedule(&mut self) -> Option<Arc<RwLock<Process>>> {
        // Simple round-robin scheduling
        if let Some(current) = self.current.take() {
            let mut process = current.write();
            if process.state != ProcessState::Terminated {
                process.state = ProcessState::Ready;
                self.processes.push(Arc::clone(&current));
            }
        }

        self.current = self.processes.pop();
        if let Some(ref process) = self.current {
            process.write().state = ProcessState::Running;
        }
        self.current.clone()
    }
}

lazy_static! {
    pub static ref PROCESS_MANAGER: RwLock<ProcessManager> = RwLock::new(ProcessManager::new());
}

pub fn init() {
    println!("Initializing process manager...");
    
    // Initialize system calls
    syscall::init();
    
    println!("Process manager initialized successfully!");
} 