#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}Setting up RustOS development environment...${NC}"

# Check if Rust is installed
if ! command -v rustc &> /dev/null; then
    echo -e "${RED}Rust not found. Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
else
    echo -e "${GREEN}Rust is already installed${NC}"
fi

# Install required components and tools
echo -e "${BLUE}Installing required components...${NC}"
rustup override set nightly
rustup component add rust-src
rustup component add llvm-tools-preview

# Install cargo-bootimage
echo -e "${BLUE}Installing cargo-bootimage...${NC}"
cargo install cargo-bootimage

# Check if QEMU is installed
if ! command -v qemu-system-x86_64 &> /dev/null; then
    echo -e "${RED}QEMU not found. Please install QEMU:${NC}"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "Run: brew install qemu"
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "Run: sudo apt-get install qemu-system-x86"
    else
        echo "Please install QEMU for your system"
    fi
    exit 1
else
    echo -e "${GREEN}QEMU is already installed${NC}"
fi

# Build the OS
echo -e "${BLUE}Building RustOS...${NC}"
cargo build

# Create bootable image
echo -e "${BLUE}Creating bootable image...${NC}"
cargo bootimage

echo -e "${GREEN}Setup complete!${NC}"
echo -e "${BLUE}To run RustOS, execute: ./run.sh${NC}" 