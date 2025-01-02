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
use task::sync::Semaphore;
use core::sync::atomic::{AtomicUsize, Ordering};
use futures_util::{StreamExt, FutureExt};
use memory::heap::init_heap;
use lazy_static::lazy_static;

lazy_static! {
    static ref PRINT_SEMAPHORE: Semaphore = Semaphore::new(1);
}

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    interrupts::hlt_loop();
}

// Shared counter for testing synchronization
static SHARED_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn high_priority_task() {
    let mut local_counter = 0;
    loop {
        PRINT_SEMAPHORE.acquire();
        println!("High Priority Task: {}", local_counter);
        PRINT_SEMAPHORE.release();
        
        SHARED_COUNTER.fetch_add(1, Ordering::SeqCst);
        local_counter += 1;
        
        if local_counter > 5 {
            break;
        }
        
        for _ in 0..100000 { core::hint::spin_loop(); }
    }
}

fn normal_priority_task() {
    let mut local_counter = 0;
    loop {
        PRINT_SEMAPHORE.acquire();
        println!("Normal Priority Task: {}", local_counter);
        PRINT_SEMAPHORE.release();
        
        SHARED_COUNTER.fetch_add(1, Ordering::SeqCst);
        local_counter += 1;
        
        if local_counter > 5 {
            break;
        }
        
        for _ in 0..100000 { core::hint::spin_loop(); }
    }
}

fn low_priority_task() {
    let mut local_counter = 0;
    loop {
        PRINT_SEMAPHORE.acquire();
        println!("Low Priority Task: {}", local_counter);
        PRINT_SEMAPHORE.release();
        
        SHARED_COUNTER.fetch_add(1, Ordering::SeqCst);
        local_counter += 1;
        
        if local_counter > 5 {
            break;
        }
        
        for _ in 0..100000 { core::hint::spin_loop(); }
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
    init_heap(&mut mapper, &mut frame_allocator)
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
    
    // Spawn test tasks with different priorities
    task::spawn_with_priority(high_priority_task, task::TaskPriority::High);
    task::spawn_with_priority(normal_priority_task, task::TaskPriority::Normal);
    task::spawn_with_priority(low_priority_task, task::TaskPriority::Low);
    
    println!("Test tasks spawned successfully!");
    println!("Starting scheduler...");

    // Create a keyboard event stream
    let mut keyboard_events = keyboard::KeyboardStream::new();
    
    loop {
        if let Some(event) = keyboard_events.next().now_or_never().flatten() {
            match event {
                keyboard::KeyEvent::Char(c) => print!("{}", c),
                keyboard::KeyEvent::SpecialKey(key) => print!("{:?}", key),
            }
        }
        task::yield_now();
    }
}