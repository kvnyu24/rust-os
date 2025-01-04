use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;

pub mod driver;
pub mod ip;
pub mod tcp;
pub mod ethernet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const fn new(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpAddress([u8; 4]);

impl IpAddress {
    pub const fn new(bytes: [u8; 4]) -> Self {
        IpAddress(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for IpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

#[derive(Debug)]
pub struct NetworkInterface {
    mac_address: MacAddress,
    ip_address: IpAddress,
    netmask: IpAddress,
    gateway: IpAddress,
    rx_buffer: Vec<u8>,
    tx_buffer: Vec<u8>,
}

impl NetworkInterface {
    pub fn new(
        mac_address: MacAddress,
        ip_address: IpAddress,
        netmask: IpAddress,
        gateway: IpAddress,
    ) -> Self {
        NetworkInterface {
            mac_address,
            ip_address,
            netmask,
            gateway,
            rx_buffer: Vec::new(),
            tx_buffer: Vec::new(),
        }
    }

    pub fn mac_address(&self) -> MacAddress {
        self.mac_address
    }

    pub fn ip_address(&self) -> IpAddress {
        self.ip_address
    }

    pub fn receive(&mut self, data: &[u8]) {
        self.rx_buffer.extend_from_slice(data);
        // Process received data through the network stack
        self.process_rx_buffer();
    }

    pub fn send(&mut self, data: &[u8]) {
        self.tx_buffer.extend_from_slice(data);
        // Process and send data through the network stack
        self.process_tx_buffer();
    }

    fn process_rx_buffer(&mut self) {
        // TODO: Implement packet processing
        // 1. Parse Ethernet frame
        // 2. Handle ARP/IP packets
        // 3. Process TCP/UDP segments
        self.rx_buffer.clear();
    }

    fn process_tx_buffer(&mut self) {
        // TODO: Implement packet sending
        // 1. Build Ethernet frame
        // 2. Add IP header
        // 3. Add TCP/UDP header
        // 4. Send through network driver
        self.tx_buffer.clear();
    }
}

lazy_static! {
    pub static ref NETWORK_INTERFACE: Mutex<Option<NetworkInterface>> = Mutex::new(None);
}

pub fn init() {
    let interface = NetworkInterface::new(
        MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]), // QEMU default MAC
        IpAddress::new([192, 168, 1, 10]),                      // Default IP
        IpAddress::new([255, 255, 255, 0]),                     // Subnet mask
        IpAddress::new([192, 168, 1, 1]),                       // Gateway
    );
    *NETWORK_INTERFACE.lock() = Some(interface);
} 