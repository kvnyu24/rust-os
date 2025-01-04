use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::network::{IpAddress, ip::{IpPacket, IpProtocol}};

const UDP_HEADER_LEN: usize = 8;

#[derive(Debug)]
pub struct UdpPacket {
    source_port: u16,
    destination_port: u16,
    length: u16,
    checksum: u16,
    payload: Vec<u8>,
}

impl UdpPacket {
    pub fn new(source_port: u16, destination_port: u16, payload: Vec<u8>) -> Self {
        let length = (UDP_HEADER_LEN + payload.len()) as u16;
        UdpPacket {
            source_port,
            destination_port,
            length,
            checksum: 0,  // Will be calculated
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < UDP_HEADER_LEN {
            return None;
        }

        let source_port = u16::from_be_bytes([data[0], data[1]]);
        let destination_port = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let checksum = u16::from_be_bytes([data[6], data[7]]);
        let payload = data[UDP_HEADER_LEN..].to_vec();

        Some(UdpPacket {
            source_port,
            destination_port,
            length,
            checksum,
            payload,
        })
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(UDP_HEADER_LEN + self.payload.len());

        // Source port
        bytes.extend_from_slice(&self.source_port.to_be_bytes());

        // Destination port
        bytes.extend_from_slice(&self.destination_port.to_be_bytes());

        // Length
        bytes.extend_from_slice(&self.length.to_be_bytes());

        // Checksum (initially 0)
        bytes.extend_from_slice(&[0, 0]);

        // Payload
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    pub fn calculate_checksum(&mut self, source_ip: IpAddress, dest_ip: IpAddress) {
        // UDP checksum includes a pseudo-header with IP addresses
        let mut sum: u32 = 0;

        // Add source IP
        for byte in source_ip.as_bytes().chunks(2) {
            sum += u16::from_be_bytes([byte[0], byte[1]]) as u32;
        }

        // Add destination IP
        for byte in dest_ip.as_bytes().chunks(2) {
            sum += u16::from_be_bytes([byte[0], byte[1]]) as u32;
        }

        // Add protocol number (17 for UDP) and UDP length
        sum += 17u32;
        sum += self.length as u32;

        // Add UDP header and data
        let packet_bytes = self.to_bytes();
        for chunk in packet_bytes.chunks(2) {
            let value = if chunk.len() == 2 {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], 0])
            } as u32;
            sum += value;
        }

        // Add carried bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        // One's complement
        self.checksum = !sum as u16;
    }
}

type PortNumber = u16;
type UdpCallback = Box<dyn Fn(&[u8], IpAddress, PortNumber) + Send>;

lazy_static! {
    static ref UDP_SOCKETS: Mutex<BTreeMap<PortNumber, UdpCallback>> = Mutex::new(BTreeMap::new());
}

pub fn bind(port: PortNumber, callback: UdpCallback) -> Result<(), &'static str> {
    let mut sockets = UDP_SOCKETS.lock();
    if sockets.contains_key(&port) {
        return Err("Port already in use");
    }
    sockets.insert(port, callback);
    Ok(())
}

pub fn unbind(port: PortNumber) {
    UDP_SOCKETS.lock().remove(&port);
}

pub fn send(
    source_port: PortNumber,
    destination_ip: IpAddress,
    destination_port: PortNumber,
    data: &[u8],
) -> Result<(), &'static str> {
    let mut packet = UdpPacket::new(source_port, destination_port, data.to_vec());
    
    // Create IP packet
    let mut ip_packet = IpPacket::new(
        crate::network::NETWORK_INTERFACE.lock().as_ref().unwrap().ip_address(),
        destination_ip,
        IpProtocol::Udp,
        packet.to_bytes(),
    );

    // Send through network interface
    if let Some(interface) = &mut *crate::network::NETWORK_INTERFACE.lock() {
        interface.send(&ip_packet.to_bytes());
        Ok(())
    } else {
        Err("Network interface not initialized")
    }
}

pub fn handle_udp_packet(packet: UdpPacket, source_ip: IpAddress) {
    if let Some(callback) = UDP_SOCKETS.lock().get(&packet.destination_port) {
        callback(&packet.payload, source_ip, packet.source_port);
    }
} 