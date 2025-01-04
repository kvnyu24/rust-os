use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;

pub mod driver;
pub mod ip;
pub mod tcp;
pub mod udp;
pub mod icmp;
pub mod arp;
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
        if let Some(frame) = ethernet::EthernetFrame::parse(&self.rx_buffer) {
            match frame.ethertype() {
                ethernet::EtherType::Arp => {
                    if let Some(arp_packet) = arp::ArpPacket::parse(frame.payload()) {
                        arp::handle_arp_packet(arp_packet);
                    }
                }
                ethernet::EtherType::Ipv4 => {
                    if let Some(ip_packet) = ip::IpPacket::parse(frame.payload()) {
                        match ip_packet.protocol() {
                            ip::IpProtocol::Icmp => {
                                if let Some(icmp_packet) = icmp::IcmpPacket::parse(ip_packet.payload()) {
                                    icmp::handle_icmp_packet(icmp_packet, ip_packet.source());
                                }
                            }
                            ip::IpProtocol::Tcp => {
                                if let Some(tcp_segment) = tcp::TcpSegment::parse(ip_packet.payload()) {
                                    // Handle TCP segment
                                }
                            }
                            ip::IpProtocol::Udp => {
                                if let Some(udp_packet) = udp::UdpPacket::parse(ip_packet.payload()) {
                                    udp::handle_udp_packet(udp_packet, ip_packet.source());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        self.rx_buffer.clear();
    }

    fn process_tx_buffer(&mut self) {
        if let Some(driver) = &mut *driver::NETWORK_DRIVER.lock() {
            let _ = driver.send(&self.tx_buffer);
        }
        self.tx_buffer.clear();
    }

    pub fn send_ip(&mut self, packet: &ip::IpPacket) -> Result<(), &'static str> {
        // Get destination MAC address through ARP
        let dest_mac = if let Some(mac) = arp::get_mac_address(packet.destination()) {
            mac
        } else {
            return Err("Could not resolve MAC address");
        };

        // Create Ethernet frame
        let frame = ethernet::EthernetFrame::new(
            dest_mac,
            self.mac_address,
            ethernet::EtherType::Ipv4,
            packet.to_bytes(),
        );

        self.tx_buffer.extend_from_slice(&frame.to_bytes());
        self.process_tx_buffer();
        Ok(())
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