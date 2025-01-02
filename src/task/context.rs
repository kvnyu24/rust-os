#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TaskContext {
    // Callee-saved registers
    rsp: usize,    // Stack pointer
    r15: usize,
    r14: usize,
    r13: usize,
    r12: usize,
    rbx: usize,
    rbp: usize,    // Base pointer
    rip: usize,    // Instruction pointer
}

impl TaskContext {
    pub fn new(entry_point: usize, stack_top: usize) -> Self {
        Self {
            rsp: stack_top,
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            rip: entry_point,
        }
    }

    pub fn switch(&mut self, next: &mut TaskContext) {
        unsafe {
            switch_context_inner(self, next);
        }
    }
}

#[naked]
unsafe extern "C" fn switch_context_inner(_current: *mut TaskContext, _next: *mut TaskContext) {
    use core::arch::asm;
    asm!(
        // Save current context
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi + 0], rsp",  // Save RSP

        // Load next context
        "mov rsp, [rsi + 0]",  // Restore RSP
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",
        options(noreturn)
    );
}

pub unsafe fn switch_context() {
    use super::SCHEDULER;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        if let Some(next_task) = SCHEDULER.lock().schedule() {
            let mut next = next_task.write();
            let mut current = SCHEDULER.lock().current.as_ref().unwrap().write();
            current.context.switch(&mut next.context);
        }
    });
} 