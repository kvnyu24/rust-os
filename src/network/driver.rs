use alloc::vec::Vec;
use spin::Mutex;
use x86_64::instructions::port::Port;
use crate::network::{MacAddress, NETWORK_INTERFACE};

pub trait NetworkDriver: Send {
    fn init(&mut self) -> Result<(), &'static str>;
    fn send(&mut self, data: &[u8]) -> Result<(), &'static str>;
    fn receive(&mut self) -> Option<Vec<u8>>;
    fn mac_address(&self) -> MacAddress;
}

// Basic implementation for QEMU's RTL8139 network card
pub struct Rtl8139 {
    io_base: u16,
    mac_address: MacAddress,
    rx_buffer: Vec<u8>,
    tx_buffer: [Vec<u8>; 4],
    current_tx_buffer: usize,
}

const RTL8139_CMD: u16 = 0x37;
const RTL8139_IMR: u16 = 0x3C;
const RTL8139_RCR: u16 = 0x44;
const RTL8139_CONFIG_1: u16 = 0x52;

impl Rtl8139 {
    pub fn new(io_base: u16) -> Self {
        Rtl8139 {
            io_base,
            mac_address: MacAddress::new([0; 6]),
            rx_buffer: Vec::with_capacity(8192),
            tx_buffer: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            current_tx_buffer: 0,
        }
    }

    fn read_mac_address(&mut self) {
        let mut mac = [0u8; 6];
        for i in 0..6 {
            let mut port = Port::new(self.io_base + i as u16);
            unsafe {
                mac[i] = port.read();
            }
        }
        self.mac_address = MacAddress::new(mac);
    }
}

impl NetworkDriver for Rtl8139 {
    fn init(&mut self) -> Result<(), &'static str> {
        unsafe {
            // Power on
            let mut port = Port::new(self.io_base + RTL8139_CONFIG_1);
            port.write(0x00u8);

            // Software reset
            let mut cmd_port = Port::new(self.io_base + RTL8139_CMD);
            cmd_port.write(0x10u8);

            // Wait for reset to complete
            while (cmd_port.read() & 0x10) != 0 {}

            // Enable receive and transmit
            cmd_port.write(0x0Cu8);

            // Configure receive buffer
            let mut rcr_port = Port::new(self.io_base + RTL8139_RCR);
            rcr_port.write(0x0Fu32);

            // Configure interrupts
            let mut imr_port = Port::new(self.io_base + RTL8139_IMR);
            imr_port.write(0x0005u16);

            self.read_mac_address();
        }

        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > 1792 {
            return Err("Packet too large");
        }

        // Copy data to current transmit buffer
        self.tx_buffer[self.current_tx_buffer].clear();
        self.tx_buffer[self.current_tx_buffer].extend_from_slice(data);

        unsafe {
            // Write packet address and size
            let tx_addr = self.tx_buffer[self.current_tx_buffer].as_ptr() as u32;
            let mut tx_status_port = Port::new(self.io_base + 0x10 + self.current_tx_buffer as u16 * 4);
            tx_status_port.write(tx_addr);

            let mut tx_cmd_port = Port::new(self.io_base + 0x10 + self.current_tx_buffer as u16 * 4 + 4);
            tx_cmd_port.write((data.len() as u32) & 0x1FFF);
        }

        // Move to next buffer
        self.current_tx_buffer = (self.current_tx_buffer + 1) % 4;

        Ok(())
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        unsafe {
            let mut cmd_port = Port::new(self.io_base + RTL8139_CMD);
            if (cmd_port.read() & 0x01) == 0 {
                return None;
            }

            // Read packet size and data
            let mut size_port = Port::new(self.io_base + 0x30);
            let size = size_port.read() as usize;

            if size == 0 {
                return None;
            }

            self.rx_buffer.clear();
            for _ in 0..size {
                let mut data_port = Port::new(self.io_base + 0x30);
                self.rx_buffer.push(data_port.read());
            }

            // Update read pointer
            cmd_port.write(0x01u8);

            Some(self.rx_buffer.clone())
        }
    }

    fn mac_address(&self) -> MacAddress {
        self.mac_address
    }
}

lazy_static! {
    pub static ref NETWORK_DRIVER: Mutex<Option<Box<dyn NetworkDriver>>> = Mutex::new(None);
}

pub fn init() -> Result<(), &'static str> {
    let mut driver = Rtl8139::new(0xC000); // Default I/O base for QEMU
    driver.init()?;

    // Store MAC address in network interface
    if let Some(interface) = &mut *NETWORK_INTERFACE.lock() {
        interface.mac_address = driver.mac_address();
    }

    *NETWORK_DRIVER.lock() = Some(Box::new(driver));
    Ok(())
} 