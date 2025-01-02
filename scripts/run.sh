#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building RustOS...${NC}"
cargo build || { echo -e "${RED}Build failed${NC}"; exit 1; }

echo -e "${BLUE}Creating bootable image...${NC}"
cargo bootimage || { echo -e "${RED}Bootimage creation failed${NC}"; exit 1; }

echo -e "${BLUE}Running RustOS in QEMU...${NC}"
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-rust_os/debug/bootimage-rust-os.bin \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -serial stdio \
    -display gtk \
    -machine type=q35 