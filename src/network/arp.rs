use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::network::{MacAddress, IpAddress, ethernet::{EthernetFrame, EtherType}, NETWORK_INTERFACE};
use crate::network::driver::NETWORK_DRIVER;

const ARP_HARDWARE_TYPE_ETHERNET: u16 = 1;
const ARP_PROTOCOL_TYPE_IPV4: u16 = 0x0800;
const ARP_HARDWARE_SIZE: u8 = 6;  // MAC address size
const ARP_PROTOCOL_SIZE: u8 = 4;  // IPv4 address size
const ARP_CACHE_TIMEOUT: u64 = 300;  // 5 minutes in seconds

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ArpOperation {
    Request = 1,
    Reply = 2,
}

impl From<u16> for ArpOperation {
    fn from(value: u16) -> Self {
        match value {
            1 => ArpOperation::Request,
            2 => ArpOperation::Reply,
            _ => ArpOperation::Request,
        }
    }
}

#[derive(Debug)]
pub struct ArpPacket {
    hardware_type: u16,
    protocol_type: u16,
    hardware_size: u8,
    protocol_size: u8,
    operation: ArpOperation,
    sender_mac: MacAddress,
    sender_ip: IpAddress,
    target_mac: MacAddress,
    target_ip: IpAddress,
}

impl ArpPacket {
    pub fn new_request(sender_mac: MacAddress, sender_ip: IpAddress, target_ip: IpAddress) -> Self {
        ArpPacket {
            hardware_type: ARP_HARDWARE_TYPE_ETHERNET,
            protocol_type: ARP_PROTOCOL_TYPE_IPV4,
            hardware_size: ARP_HARDWARE_SIZE,
            protocol_size: ARP_PROTOCOL_SIZE,
            operation: ArpOperation::Request,
            sender_mac,
            sender_ip,
            target_mac: MacAddress::new([0; 6]),
            target_ip,
        }
    }

    pub fn new_reply(sender_mac: MacAddress, sender_ip: IpAddress, target_mac: MacAddress, target_ip: IpAddress) -> Self {
        ArpPacket {
            hardware_type: ARP_HARDWARE_TYPE_ETHERNET,
            protocol_type: ARP_PROTOCOL_TYPE_IPV4,
            hardware_size: ARP_HARDWARE_SIZE,
            protocol_size: ARP_PROTOCOL_SIZE,
            operation: ArpOperation::Reply,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {  // Minimum ARP packet size
            return None;
        }

        let hardware_type = u16::from_be_bytes([data[0], data[1]]);
        let protocol_type = u16::from_be_bytes([data[2], data[3]]);
        let hardware_size = data[4];
        let protocol_size = data[5];
        let operation = ArpOperation::from(u16::from_be_bytes([data[6], data[7]]));

        let mut mac_bytes = [0u8; 6];
        let mut ip_bytes = [0u8; 4];

        // Parse sender MAC
        mac_bytes.copy_from_slice(&data[8..14]);
        let sender_mac = MacAddress::new(mac_bytes);

        // Parse sender IP
        ip_bytes.copy_from_slice(&data[14..18]);
        let sender_ip = IpAddress::new(ip_bytes);

        // Parse target MAC
        mac_bytes.copy_from_slice(&data[18..24]);
        let target_mac = MacAddress::new(mac_bytes);

        // Parse target IP
        ip_bytes.copy_from_slice(&data[24..28]);
        let target_ip = IpAddress::new(ip_bytes);

        Some(ArpPacket {
            hardware_type,
            protocol_type,
            hardware_size,
            protocol_size,
            operation,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(28);

        // Hardware type
        bytes.extend_from_slice(&self.hardware_type.to_be_bytes());

        // Protocol type
        bytes.extend_from_slice(&self.protocol_type.to_be_bytes());

        // Hardware and protocol sizes
        bytes.push(self.hardware_size);
        bytes.push(self.protocol_size);

        // Operation
        bytes.extend_from_slice(&(self.operation as u16).to_be_bytes());

        // Sender MAC and IP
        bytes.extend_from_slice(self.sender_mac.as_bytes());
        bytes.extend_from_slice(self.sender_ip.as_bytes());

        // Target MAC and IP
        bytes.extend_from_slice(self.target_mac.as_bytes());
        bytes.extend_from_slice(self.target_ip.as_bytes());

        bytes
    }
}

#[derive(Debug)]
struct ArpCacheEntry {
    mac_address: MacAddress,
    timestamp: u64,
}

lazy_static! {
    static ref ARP_CACHE: Mutex<BTreeMap<IpAddress, ArpCacheEntry>> = Mutex::new(BTreeMap::new());
}

pub fn handle_arp_packet(packet: ArpPacket) {
    match packet.operation {
        ArpOperation::Request => handle_arp_request(packet),
        ArpOperation::Reply => handle_arp_reply(packet),
    }
}

fn handle_arp_request(packet: ArpPacket) {
    if let Some(interface) = &*NETWORK_INTERFACE.lock() {
        if packet.target_ip == interface.ip_address() {
            // Send ARP reply
            let reply = ArpPacket::new_reply(
                interface.mac_address(),
                interface.ip_address(),
                packet.sender_mac,
                packet.sender_ip,
            );

            let frame = EthernetFrame::new(
                packet.sender_mac,
                interface.mac_address(),
                EtherType::Arp,
                reply.to_bytes(),
            );

            if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
                let _ = driver.send(&frame.to_bytes());
            }
        }
    }
}

fn handle_arp_reply(packet: ArpPacket) {
    // Update ARP cache
    ARP_CACHE.lock().insert(packet.sender_ip, ArpCacheEntry {
        mac_address: packet.sender_mac,
        timestamp: get_current_time(),
    });
}

pub fn get_mac_address(ip: IpAddress) -> Option<MacAddress> {
    // Check cache first
    let mut cache = ARP_CACHE.lock();
    if let Some(entry) = cache.get(&ip) {
        if get_current_time() - entry.timestamp < ARP_CACHE_TIMEOUT {
            return Some(entry.mac_address);
        }
        cache.remove(&ip);
    }

    // Send ARP request
    if let Some(interface) = &*NETWORK_INTERFACE.lock() {
        let request = ArpPacket::new_request(
            interface.mac_address(),
            interface.ip_address(),
            ip,
        );

        let frame = EthernetFrame::new(
            MacAddress::new([0xFF; 6]),  // Broadcast
            interface.mac_address(),
            EtherType::Arp,
            request.to_bytes(),
        );

        if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
            let _ = driver.send(&frame.to_bytes());
        }
    }

    None  // MAC address not found
}

// TODO: Implement proper time source
fn get_current_time() -> u64 {
    0  // Placeholder, should return system time in seconds
} 