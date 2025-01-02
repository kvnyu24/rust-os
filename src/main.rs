#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(default_alloc_error_handler)]

extern crate alloc;

mod vga_buffer;
mod gdt;
mod interrupts;
mod memory;
mod keyboard;
mod task;
mod fs;
mod process;

use bootloader::BootInfo;
use core::panic::PanicInfo;
use x86_64::VirtAddr;
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc, string::String};
use futures_util::StreamExt;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    interrupts::hlt_loop();
}

fn task1() {
    let mut i = 0;
    loop {
        println!("Task 1: {}", i);
        i += 1;
        for _ in 0..1000000 { core::hint::spin_loop(); }
    }
}

fn task2() {
    let mut i = 0;
    loop {
        println!("Task 2: {}", i);
        i += 1;
        for _ in 0..1000000 { core::hint::spin_loop(); }
    }
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    println!("Hello World!");
    println!("Welcome to RustOS!");
    println!("---------------");
    println!("Initializing GDT...");
    
    gdt::init();
    
    println!("GDT initialized successfully!");
    println!("Initializing interrupts...");
    
    interrupts::init();
    
    println!("Interrupts initialized successfully!");
    println!("Initializing memory management...");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // Initialize heap
    memory::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    println!("Memory management initialized!");
    println!("Initializing filesystem...");
    
    // Initialize filesystem
    fs::init();
    
    println!("Filesystem initialized successfully!");
    println!("Initializing process manager...");
    
    // Initialize process manager
    process::init();
    
    println!("Process manager initialized successfully!");
    println!("Initializing keyboard...");
    
    // Initialize keyboard
    keyboard::init();
    
    println!("Keyboard initialized successfully!");
    println!("Initializing task scheduler...");
    
    // Initialize task scheduler
    task::init();
    
    // Create some test files and directories
    let _ = fs::ROOT_FS.read().create_dir("/bin");
    let _ = fs::ROOT_FS.read().create_dir("/home");
    let _ = fs::ROOT_FS.read().create_file("/home/welcome.txt", b"Welcome to RustOS!\n".to_vec());
    
    // Spawn test tasks
    task::spawn(task1);
    task::spawn(task2);
    
    // Create a test user process
    let test_program = b"\
        mov rax, 1      // write syscall
        mov rdi, 1      // stdout
        mov rsi, msg    // message
        mov rdx, 14     // length
        int 0x80        // syscall
        mov rax, 0      // exit syscall
        int 0x80        // syscall
        msg: db 'Hello, World!',0xa
    ".to_vec();
    
    if let Ok(pid) = process::PROCESS_MANAGER.write().spawn(
        String::from("test_process"),
        test_program,
    ) {
        println!("Spawned test process with PID: {}", pid);
    }
    
    println!("System initialization complete!");
    println!("Starting main loop...");

    // Create a keyboard event stream
    let mut keyboard_events = keyboard::KeyboardStream::new();
    
    loop {
        if let Some(event) = futures_util::executor::block_on(keyboard_events.next()) {
            match event {
                keyboard::KeyEvent::Char(c) => print!("{}", c),
                keyboard::KeyEvent::SpecialKey(key) => print!("{:?}", key),
            }
        }
        task::yield_now();
    }
}