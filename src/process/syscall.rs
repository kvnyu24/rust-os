use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use lazy_static::lazy_static;
use alloc::{string::String, vec::Vec};
use core::arch::asm;
use crate::{fs, println};

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum SyscallNumber {
    Exit = 0,
    Write = 1,
    Read = 2,
    Open = 3,
    Close = 4,
    CreateFile = 5,
    CreateDir = 6,
    Remove = 7,
    Spawn = 8,
    GetPid = 9,
}

const SYSCALL_INTERRUPT: u8 = 0x80;

lazy_static! {
    static ref SYSCALL_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt[SYSCALL_INTERRUPT as usize].set_handler_fn(syscall_handler);
        }
        idt
    };
}

pub fn init() {
    unsafe {
        SYSCALL_IDT.load();
    }
}

pub fn init_process_context() {
    // Set up any necessary process-specific context for system calls
}

extern "x86-interrupt" fn syscall_handler(stack_frame: InterruptStackFrame) {
    // System call arguments are passed in registers:
    // rax: syscall number
    // rdi: arg1
    // rsi: arg2
    // rdx: arg3
    // r10: arg4
    // r8:  arg5
    // r9:  arg6
    
    let syscall_number: usize;
    let arg1: usize;
    let arg2: usize;
    let arg3: usize;
    
    unsafe {
        asm!(
            "mov {0}, rax",
            "mov {1}, rdi",
            "mov {2}, rsi",
            "mov {3}, rdx",
            out(reg) syscall_number,
            out(reg) arg1,
            out(reg) arg2,
            out(reg) arg3,
        );
    }

    let result = match syscall_number.try_into().unwrap_or(SyscallNumber::Exit) {
        SyscallNumber::Exit => sys_exit(arg1 as i32),
        SyscallNumber::Write => sys_write(arg1, arg2 as *const u8, arg3),
        SyscallNumber::Read => sys_read(arg1, arg2 as *mut u8, arg3),
        SyscallNumber::Open => sys_open(arg1 as *const u8, arg2),
        SyscallNumber::Close => sys_close(arg1),
        SyscallNumber::CreateFile => sys_create_file(arg1 as *const u8),
        SyscallNumber::CreateDir => sys_create_dir(arg1 as *const u8),
        SyscallNumber::Remove => sys_remove(arg1 as *const u8),
        SyscallNumber::Spawn => sys_spawn(arg1 as *const u8),
        SyscallNumber::GetPid => sys_getpid(),
    };

    // Return value goes in rax
    unsafe {
        asm!(
            "mov rax, {0}",
            in(reg) result,
        );
    }
}

fn sys_exit(status: i32) -> usize {
    println!("Process exited with status: {}", status);
    0
}

fn sys_write(fd: usize, buf: *const u8, count: usize) -> usize {
    let slice = unsafe { core::slice::from_raw_parts(buf, count) };
    match fd {
        1 => { // stdout
            print!("{}", core::str::from_utf8(slice).unwrap_or("Invalid UTF-8"));
            count
        }
        2 => { // stderr
            print!("{}", core::str::from_utf8(slice).unwrap_or("Invalid UTF-8"));
            count
        }
        _ => {
            // Handle regular file writes
            0
        }
    }
}

fn sys_read(fd: usize, buf: *mut u8, count: usize) -> usize {
    0 // TODO: Implement actual file reading
}

fn sys_open(path: *const u8, flags: usize) -> usize {
    0 // TODO: Implement file opening
}

fn sys_close(fd: usize) -> usize {
    0 // TODO: Implement file closing
}

fn sys_create_file(path: *const u8) -> usize {
    let path_str = unsafe {
        let path = core::slice::from_raw_parts(path, 1024); // Max path length
        let len = path.iter().position(|&c| c == 0).unwrap_or(1024);
        core::str::from_utf8(&path[..len]).unwrap_or("")
    };
    
    match fs::ROOT_FS.read().create_file(path_str, Vec::new()) {
        Ok(()) => 0,
        Err(_) => usize::MAX,
    }
}

fn sys_create_dir(path: *const u8) -> usize {
    let path_str = unsafe {
        let path = core::slice::from_raw_parts(path, 1024);
        let len = path.iter().position(|&c| c == 0).unwrap_or(1024);
        core::str::from_utf8(&path[..len]).unwrap_or("")
    };
    
    match fs::ROOT_FS.read().create_dir(path_str) {
        Ok(()) => 0,
        Err(_) => usize::MAX,
    }
}

fn sys_remove(path: *const u8) -> usize {
    let path_str = unsafe {
        let path = core::slice::from_raw_parts(path, 1024);
        let len = path.iter().position(|&c| c == 0).unwrap_or(1024);
        core::str::from_utf8(&path[..len]).unwrap_or("")
    };
    
    match fs::ROOT_FS.read().remove(path_str) {
        Ok(()) => 0,
        Err(_) => usize::MAX,
    }
}

fn sys_spawn(path: *const u8) -> usize {
    let path_str = unsafe {
        let path = core::slice::from_raw_parts(path, 1024);
        let len = path.iter().position(|&c| c == 0).unwrap_or(1024);
        core::str::from_utf8(&path[..len]).unwrap_or("")
    };
    
    // TODO: Load program from filesystem and spawn process
    0
}

fn sys_getpid() -> usize {
    super::PROCESS_MANAGER.read()
        .current_process()
        .map(|p| p.read().id())
        .unwrap_or(0)
} 