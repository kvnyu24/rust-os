# RustOS

A bare metal operating system written in Rust, targeting x86_64 architecture.

## Project Overview

This project is a minimal operating system implemented in Rust, running directly on x86_64 hardware. It demonstrates low-level OS concepts while leveraging Rust's safety features.

## Features (Roadmap)

- [x] Bare metal environment setup
- [x] Basic VGA text buffer output
- [x] Global Descriptor Table (GDT)
- [x] Interrupt Descriptor Table (IDT)
- [x] Keyboard input handling
- [x] Memory management
  - [x] Physical memory management
  - [x] Virtual memory & paging
- [x] Heap allocation
- [x] Multi-threading support
- [x] Basic filesystem
  - [ ] File permissions and ownership
  - [ ] Directory traversal
  - [ ] File read/write buffering
  - [ ] Basic disk driver for persistence
- [x] User space programs
- [x] Shell Environment
  - [x] Command-line interface (ls, cd, cat, etc.)
  - [x] Command history
  - [x] Tab completion
  - [x] File redirection (>, >>, <)
  - [x] Command piping (|)
  - [ ] Basic shell scripting
- [x] Network Stack
  - [x] Network driver (RTL8139)
  - [x] TCP/IP implementation
  - [ ] Basic network services (DHCP, DNS)
  - [ ] Socket interface
  - [ ] Network utilities (ping, netstat)
- [ ] Advanced Process Management
  - [ ] Inter-process communication (IPC)
  - [ ] Enhanced process isolation
  - [ ] Extended system calls
- [ ] Device Management
  - [ ] Device driver framework
  - [ ] Additional hardware support
  - [ ] Power management

## Project Structure

```bash
rust-os/
├── src/
│   ├── main.rs              # Kernel entry point
│   ├── vga_buffer.rs        # VGA text mode driver (TODO)
│   ├── interrupts/          # CPU exception and hardware interrupt handlers (TODO)
│   ├── memory/              # Memory management code (TODO)
│   └── lib.rs              # Kernel library code (TODO)
├── .cargo/
│   └── config.toml         # Cargo configuration
├── x86_64-rust_os.json     # Custom target specification
├── rust-toolchain.toml     # Rust toolchain configuration
├── Cargo.toml              # Project dependencies
└── README.md              # This file
```

## Prerequisites

- Rust (nightly)
- QEMU (for testing)
- cargo-xbuild
- bootimage

## Building

1. Install required tools:

```bash
rustup component add rust-src
cargo install cargo-xbuild bootimage
```

2. Build the kernel:

```bash
cargo build
```

3. Create bootable disk image:

```bash
cargo bootimage
```

4. Run in QEMU:

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-rust_os/debug/bootimage-rust-os.bin
```

## Development Phases

1. **Phase 1: Bootloader and Basic Output**
   - Custom target specification
   - Bare metal environment
   - VGA text buffer output

2. **Phase 2: CPU Exceptions and Interrupts**
   - GDT implementation
   - IDT setup
   - Basic exception handling
   - Hardware interrupts

3. **Phase 3: Memory Management**
   - Physical memory detection
   - Page frame allocation
   - Virtual memory setup
   - Heap allocation

4. **Phase 4: Concurrency and Process Management**
   - Multi-threading support
   - Basic scheduler
   - Process isolation

5. **Phase 5: Drivers and User Space**
   - Keyboard driver
   - Basic filesystem
   - User space programs
   - System calls

## Contributing

Feel free to contribute by opening issues or submitting pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
