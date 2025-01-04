use alloc::vec::Vec;
use alloc::string::ToString;
use crate::network::{IpAddress, ip::{IpPacket, IpProtocol}};

const ICMP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IcmpType {
    EchoReply = 0,
    DestinationUnreachable = 3,
    EchoRequest = 8,
    TimeExceeded = 11,
}

impl From<u8> for IcmpType {
    fn from(value: u8) -> Self {
        match value {
            0 => IcmpType::EchoReply,
            3 => IcmpType::DestinationUnreachable,
            8 => IcmpType::EchoRequest,
            11 => IcmpType::TimeExceeded,
            _ => IcmpType::EchoReply,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum IcmpCode {
    // Destination Unreachable codes
    NetworkUnreachable = 0,
    HostUnreachable = 1,
    ProtocolUnreachable = 2,
    PortUnreachable = 3,
    FragmentationNeeded = 4,
    SourceRouteFailed = 5,

    // Time Exceeded codes
    TtlExceeded = 11,
    FragmentReassemblyTimeExceeded = 12,

    // Echo Request/Reply codes
    EchoCode = 8,
}

impl From<u8> for IcmpCode {
    fn from(value: u8) -> Self {
        match value {
            0 => IcmpCode::NetworkUnreachable,
            1 => IcmpCode::HostUnreachable,
            2 => IcmpCode::ProtocolUnreachable,
            3 => IcmpCode::PortUnreachable,
            4 => IcmpCode::FragmentationNeeded,
            5 => IcmpCode::SourceRouteFailed,
            11 => IcmpCode::TtlExceeded,
            12 => IcmpCode::FragmentReassemblyTimeExceeded,
            _ => IcmpCode::EchoCode,
        }
    }
}

#[derive(Debug)]
pub struct IcmpPacket {
    icmp_type: IcmpType,
    code: IcmpCode,
    checksum: u16,
    rest_of_header: u32,  // Used differently depending on ICMP type
    payload: Vec<u8>,
}

impl IcmpPacket {
    pub fn new_echo_request(identifier: u16, sequence: u16, payload: Vec<u8>) -> Self {
        let rest_of_header = ((identifier as u32) << 16) | (sequence as u32);
        IcmpPacket {
            icmp_type: IcmpType::EchoRequest,
            code: IcmpCode::EchoCode,
            checksum: 0,  // Will be calculated
            rest_of_header,
            payload,
        }
    }

    pub fn new_echo_reply(identifier: u16, sequence: u16, payload: Vec<u8>) -> Self {
        let rest_of_header = ((identifier as u32) << 16) | (sequence as u32);
        IcmpPacket {
            icmp_type: IcmpType::EchoReply,
            code: IcmpCode::EchoCode,
            checksum: 0,  // Will be calculated
            rest_of_header,
            payload,
        }
    }

    pub fn new_destination_unreachable(code: IcmpCode, original_packet: &[u8]) -> Self {
        IcmpPacket {
            icmp_type: IcmpType::DestinationUnreachable,
            code,
            checksum: 0,  // Will be calculated
            rest_of_header: 0,  // Unused for destination unreachable
            payload: original_packet[..64].to_vec(),  // First 64 bytes of original packet
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < ICMP_HEADER_LEN {
            return None;
        }

        let icmp_type = IcmpType::from(data[0]);
        let code = match data[1] {
            0..=3 if icmp_type == IcmpType::DestinationUnreachable => {
                unsafe { core::mem::transmute(data[1]) }
            }
            0..=1 if icmp_type == IcmpType::TimeExceeded => {
                unsafe { core::mem::transmute(data[1]) }
            }
            _ => IcmpCode::EchoCode,
        };
        let checksum = u16::from_be_bytes([data[2], data[3]]);
        let rest_of_header = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let payload = data[ICMP_HEADER_LEN..].to_vec();

        Some(IcmpPacket {
            icmp_type,
            code,
            checksum,
            rest_of_header,
            payload,
        })
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(ICMP_HEADER_LEN + self.payload.len());

        // Type and Code
        bytes.push(self.icmp_type as u8);
        bytes.push(self.code as u8);

        // Checksum (initially 0)
        bytes.extend_from_slice(&[0, 0]);

        // Rest of header
        bytes.extend_from_slice(&self.rest_of_header.to_be_bytes());

        // Payload
        bytes.extend_from_slice(&self.payload);

        // Calculate checksum
        self.checksum = self.calculate_checksum(&bytes);
        bytes[2..4].copy_from_slice(&self.checksum.to_be_bytes());

        bytes
    }

    fn calculate_checksum(&self, data: &[u8]) -> u16 {
        let mut sum: u32 = 0;

        // Sum up 16-bit words
        for chunk in data.chunks(2) {
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
        !sum as u16
    }

    pub fn get_identifier(&self) -> u16 {
        (self.rest_of_header >> 16) as u16
    }

    pub fn get_sequence(&self) -> u16 {
        self.rest_of_header as u16
    }

    pub fn get_type(&self) -> IcmpType {
        self.icmp_type
    }
}

pub fn send_echo_request(destination: IpAddress, identifier: u16, sequence: u16, payload: Vec<u8>) -> Result<(), &'static str> {
    let mut icmp_packet = IcmpPacket::new_echo_request(identifier, sequence, payload);
    
    // Create IP packet
    let mut ip_packet = IpPacket::new(
        crate::network::NETWORK_INTERFACE.lock().as_ref().unwrap().ip_address(),
        destination,
        IpProtocol::Icmp,
        icmp_packet.to_bytes(),
    );

    // Send through network interface
    if let Some(interface) = &mut *crate::network::NETWORK_INTERFACE.lock() {
        interface.send(&ip_packet.to_bytes());
        Ok(())
    } else {
        Err("Network interface not initialized")
    }
}

pub fn handle_icmp_packet(packet: IcmpPacket, source_ip: IpAddress) {
    match packet.icmp_type {
        IcmpType::EchoRequest => {
            // Send echo reply
            let mut reply = IcmpPacket::new_echo_reply(
                packet.get_identifier(),
                packet.get_sequence(),
                packet.payload,
            );

            let mut ip_packet = IpPacket::new(
                crate::network::NETWORK_INTERFACE.lock().as_ref().unwrap().ip_address(),
                source_ip,
                IpProtocol::Icmp,
                reply.to_bytes(),
            );

            if let Some(interface) = &mut *crate::network::NETWORK_INTERFACE.lock() {
                interface.send(&ip_packet.to_bytes());
            }
        }
        IcmpType::EchoReply => {
            // Handle ping reply (could notify waiting ping requests)
            println!("Received ping reply from {}", source_ip);
        }
        IcmpType::DestinationUnreachable => {
            println!("Destination unreachable: {:?}", packet.code);
        }
        IcmpType::TimeExceeded => {
            println!("Time exceeded: {:?}", packet.code);
        }
    }
} 