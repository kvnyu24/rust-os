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

use bootloader::BootInfo;
use core::panic::PanicInfo;
use x86_64::VirtAddr;
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};
use futures_util::StreamExt;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    interrupts::hlt_loop();
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
    println!("Initializing keyboard...");
    
    // Initialize keyboard
    keyboard::init();
    
    println!("Keyboard initialized successfully!");
    println!("You can now type on the keyboard!");
    println!("Press any key to see it echoed back...");

    // Create a keyboard event stream
    let mut keyboard_events = keyboard::KeyboardStream::new();
    
    loop {
        if let Some(event) = futures_util::executor::block_on(keyboard_events.next()) {
            match event {
                keyboard::KeyEvent::Char(c) => print!("{}", c),
                keyboard::KeyEvent::SpecialKey(key) => print!("{:?}", key),
            }
        }
    }
}