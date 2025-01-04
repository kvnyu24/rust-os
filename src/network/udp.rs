use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::network::{IpAddress, ip::{IpPacket, IpProtocol}};

const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone)]
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
            checksum: 0,  // Will be calculated later
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < UDP_HEADER_LEN {
            return None;
        }

        let source_port = u16::from_be_bytes(data[0..2].try_into().ok()?);
        let destination_port = u16::from_be_bytes(data[2..4].try_into().ok()?);
        let length = u16::from_be_bytes(data[4..6].try_into().ok()?);
        let checksum = u16::from_be_bytes(data[6..8].try_into().ok()?);

        // Validate length
        if data.len() != length as usize {
            return None;
        }

        let payload = data[UDP_HEADER_LEN..].to_vec();

        Some(UdpPacket {
            source_port,
            destination_port,
            length,
            checksum,
            payload,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.length as usize);

        bytes.extend_from_slice(&self.source_port.to_be_bytes());
        bytes.extend_from_slice(&self.destination_port.to_be_bytes());
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    pub fn calculate_checksum(&mut self, source_ip: IpAddress, dest_ip: IpAddress) {
        self.checksum = 0;
        let mut sum: u32 = 0;

        // Add source and destination IPs
        for byte in source_ip.as_bytes().chunks_exact(2) {
            sum += u16::from_be_bytes([byte[0], byte[1]]) as u32;
        }
        for byte in dest_ip.as_bytes().chunks_exact(2) {
            sum += u16::from_be_bytes([byte[0], byte[1]]) as u32;
        }

        // Add protocol and length
        sum += (IpProtocol::Udp as u32) + (self.length as u32);

        // Add UDP header and data
        let packet_bytes = self.to_bytes();
        for chunk in packet_bytes.chunks(2) {
            let value = match chunk {
                &[b1, b2] => u16::from_be_bytes([b1, b2]) as u32,
                &[b1] => u16::from_be_bytes([b1, 0]) as u32,
                _ => unreachable!(),
            };
            sum += value;
        }

        // Fold carry bits
        while sum > 0xFFFF {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        self.checksum = !sum as u16;
    }

    pub fn verify_checksum(&self, source_ip: IpAddress, dest_ip: IpAddress) -> bool {
        let mut packet = self.clone();
        packet.calculate_checksum(source_ip, dest_ip);
        self.checksum == packet.checksum
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
    let source_ip = crate::network::NETWORK_INTERFACE
        .lock()
        .as_ref()
        .ok_or("Network interface not initialized")?
        .ip_address();

    packet.calculate_checksum(source_ip, destination_ip);
    
    let mut ip_packet = IpPacket::new(
        source_ip,
        destination_ip,
        IpProtocol::Udp,
        packet.to_bytes(),
    );

    let packet_bytes = ip_packet.to_bytes();
    crate::network::NETWORK_INTERFACE
        .lock()
        .as_mut()
        .ok_or("Network interface not initialized")?
        .send(&packet_bytes);

    Ok(())
}

pub fn handle_udp_packet(packet: UdpPacket, source_ip: IpAddress) {
    if let Some(callback) = UDP_SOCKETS.lock().get(&packet.destination_port) {
        callback(&packet.payload, source_ip, packet.source_port);
    }
}