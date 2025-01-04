use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::boxed::Box;
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
pub mod dns;
pub mod utils;
pub mod socket;
pub mod test;
pub mod dhcp;

pub mod prelude {
    pub use alloc::vec;
    pub use alloc::string::{String, ToString};
    pub use alloc::boxed::Box;
    pub use crate::println;
}

pub use driver::NETWORK_DRIVER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const fn new(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    
    pub fn octets(&self) -> [u8; 6] {
        self.0
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct IpAddress {
    pub octets: [u8; 4],
}

impl IpAddress {
    pub const fn new(bytes: [u8; 4]) -> Self {
        IpAddress { octets: bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.octets
    }

    pub fn get_network_address(&self, netmask: &IpAddress) -> IpAddress {
        let mut network_octets = [0u8; 4];
        for i in 0..4 {
            network_octets[i] = self.octets[i] & netmask.octets[i];
        }
        IpAddress::new(network_octets)
    }
}

impl fmt::Display for IpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.octets[0], self.octets[1], self.octets[2], self.octets[3]
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
    pub fn new(mac_address: MacAddress) -> Self {
        NetworkInterface {
            mac_address,
            ip_address: IpAddress::new([0, 0, 0, 0]),  // Will be set by DHCP
            netmask: IpAddress::new([0, 0, 0, 0]),     // Will be set by DHCP
            gateway: IpAddress::new([0, 0, 0, 0]),      // Will be set by DHCP
            rx_buffer: Vec::with_capacity(1500), // Standard MTU size
            tx_buffer: Vec::with_capacity(1500),
        }
    }

    pub fn mac_address(&self) -> MacAddress {
        self.mac_address
    }

    pub fn ip_address(&self) -> IpAddress {
        self.ip_address
    }

    pub fn receive(&mut self, data: &[u8]) {
        self.rx_buffer.clear();
        self.rx_buffer.extend_from_slice(data);
        self.process_rx_buffer();
    }

    pub fn send(&mut self, data: &[u8]) {
        self.tx_buffer.clear();
        self.tx_buffer.extend_from_slice(data);
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
                                    tcp::handle_tcp_segment(tcp_segment, ip_packet.source(), ip_packet.destination());
                                }
                            }
                            ip::IpProtocol::Udp => {
                                if let Some(udp_packet) = udp::UdpPacket::parse(ip_packet.payload()) {
                                    if udp_packet.destination_port == 68 { // DHCP client port
                                        let _ = dhcp::handle_dhcp_packet(&udp_packet, self);
                                    } else {
                                        udp::handle_udp_packet(udp_packet, ip_packet.source());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn process_tx_buffer(&mut self) {
        if let Some(driver) = &mut *driver::NETWORK_DRIVER.lock() {
            let _ = driver.send(&self.tx_buffer);
        }
        self.tx_buffer.clear();
    }

    pub fn send_ip(&mut self, packet: &ip::IpPacket) -> Result<(), &'static str> {
        let dest_mac = if packet.destination().octets[0] == 255 {
            MacAddress::new([0xFF; 6]) // Broadcast
        } else if let Some(mac) = arp::get_mac_address(packet.destination()) {
            mac
        } else {
            return Err("Could not resolve MAC address");
        };

        let frame = ethernet::EthernetFrame::new(
            dest_mac,
            self.mac_address,
            ethernet::EtherType::Ipv4,
            packet.to_bytes(),
        );

        self.tx_buffer.clear();
        self.tx_buffer.extend_from_slice(&frame.to_bytes());
        self.process_tx_buffer();
        Ok(())
    }

    pub fn configure(&mut self, ip: IpAddress, netmask: IpAddress, gateway: IpAddress) {
        self.ip_address = ip;
        self.netmask = netmask;
        self.gateway = gateway;
    }
}

lazy_static! {
    pub static ref NETWORK_INTERFACE: Mutex<Option<NetworkInterface>> = Mutex::new(None);
}

pub fn init() {
    let interface = NetworkInterface::new(
        MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]), // QEMU default MAC
    );
    *NETWORK_INTERFACE.lock() = Some(interface);

    // Start DHCP discovery
    if let Some(interface) = &mut *NETWORK_INTERFACE.lock() {
        dhcp::start_dhcp_discovery(interface);
    }
}