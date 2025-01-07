use alloc::vec::Vec;
use crate::network::prelude::*;
use crate::network::{IpAddress, NetworkInterface};
use crate::network::socket::{Socket, SocketType};
use crate::network::udp::UdpPacket;
use core::time::Duration;

const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_SERVER_PORT: u16 = 67;
const DHCP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum DhcpMessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Decline = 4,
    Ack = 5,
    Nak = 6,
    Release = 7,
}

#[derive(Debug)]
pub struct DhcpPacket {
    op: u8,
    htype: u8,
    hlen: u8,
    hops: u8,
    xid: u32,
    secs: u16,
    flags: u16,
    ciaddr: IpAddress,
    yiaddr: IpAddress,
    siaddr: IpAddress,
    giaddr: IpAddress,
    chaddr: [u8; 16],
    options: Vec<DhcpOption>,
}

#[derive(Debug)]
pub struct DhcpOption {
    code: u8,
    length: u8,
    data: Vec<u8>,
}

impl DhcpPacket {
    pub fn new_discover(mac_addr: &[u8]) -> Self {
        let mut chaddr = [0u8; 16];
        chaddr[..6].copy_from_slice(mac_addr);

        DhcpPacket {
            op: 1, // BOOTREQUEST
            htype: 1, // Ethernet
            hlen: 6, // MAC address length
            hops: 0,
            xid: 0x12345678, // Transaction ID
            secs: 0,
            flags: 0,
            ciaddr: IpAddress::new([0, 0, 0, 0]),
            yiaddr: IpAddress::new([0, 0, 0, 0]),
            siaddr: IpAddress::new([0, 0, 0, 0]),
            giaddr: IpAddress::new([0, 0, 0, 0]),
            chaddr,
            options: vec![
                DhcpOption {
                    code: 53, // DHCP Message Type
                    length: 1,
                    data: vec![DhcpMessageType::Discover as u8],
                },
                DhcpOption {
                    code: 55, // Parameter Request List
                    length: 4,
                    data: vec![
                        1,  // Subnet Mask
                        3,  // Router
                        6,  // Domain Name Server
                        15, // Domain Name
                    ],
                },
            ],
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(576); // Minimum DHCP packet size

        bytes.push(self.op);
        bytes.push(self.htype);
        bytes.push(self.hlen);
        bytes.push(self.hops);
        bytes.extend_from_slice(&self.xid.to_be_bytes());
        bytes.extend_from_slice(&self.secs.to_be_bytes());
        bytes.extend_from_slice(&self.flags.to_be_bytes());
        bytes.extend_from_slice(self.ciaddr.as_bytes());
        bytes.extend_from_slice(self.yiaddr.as_bytes());
        bytes.extend_from_slice(self.siaddr.as_bytes());
        bytes.extend_from_slice(self.giaddr.as_bytes());
        bytes.extend_from_slice(&self.chaddr);
        
        // Add magic cookie
        bytes.extend_from_slice(&[99, 130, 83, 99]);

        // Add options
        for option in &self.options {
            bytes.push(option.code);
            bytes.push(option.length);
            bytes.extend_from_slice(&option.data);
        }

        // End option
        bytes.push(255);

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 240 {
            return None;
        }

        let mut packet = DhcpPacket {
            op: bytes[0],
            htype: bytes[1],
            hlen: bytes[2],
            hops: bytes[3],
            xid: u32::from_be_bytes(bytes[4..8].try_into().ok()?),
            secs: u16::from_be_bytes(bytes[8..10].try_into().ok()?),
            flags: u16::from_be_bytes(bytes[10..12].try_into().ok()?),
            ciaddr: IpAddress::new(bytes[12..16].try_into().ok()?),
            yiaddr: IpAddress::new(bytes[16..20].try_into().ok()?),
            siaddr: IpAddress::new(bytes[20..24].try_into().ok()?),
            giaddr: IpAddress::new(bytes[24..28].try_into().ok()?),
            chaddr: bytes[28..44].try_into().ok()?,
            options: Vec::new(),
        };

        // Parse options starting after magic cookie
        let mut i = 240;
        while i < bytes.len() {
            let code = bytes[i];
            if code == 255 { break; } // End option
            if code == 0 { i += 1; continue; } // Pad option
            
            i += 1;
            if i >= bytes.len() { break; }
            let length = bytes[i];
            i += 1;
            
            if i + length as usize > bytes.len() { break; }
            let data = bytes[i..i + length as usize].to_vec();
            i += length as usize;

            packet.options.push(DhcpOption {
                code,
                length,
                data,
            });
        }

        Some(packet)
    }

    pub fn get_message_type(&self) -> DhcpMessageType {
        for option in &self.options {
            if option.code == 53 && !option.data.is_empty() {
                return match option.data[0] {
                    1 => DhcpMessageType::Discover,
                    2 => DhcpMessageType::Offer,
                    3 => DhcpMessageType::Request,
                    4 => DhcpMessageType::Decline,
                    5 => DhcpMessageType::Ack,
                    6 => DhcpMessageType::Nak,
                    7 => DhcpMessageType::Release,
                    _ => DhcpMessageType::Discover,
                };
            }
        }
        DhcpMessageType::Discover
    }

    pub fn new_request(mac_addr: [u8; 6], requested_ip: IpAddress) -> Self {
        let mut chaddr = [0u8; 16];
        chaddr[..6].copy_from_slice(&mac_addr);

        let mut packet = DhcpPacket {
            op: 1, // BOOTREQUEST
            htype: 1, // Ethernet
            hlen: 6, // MAC address length
            hops: 0,
            xid: 0x12345678, // Transaction ID
            secs: 0,
            flags: 0,
            ciaddr: IpAddress::new([0, 0, 0, 0]),
            yiaddr: requested_ip,
            siaddr: IpAddress::new([0, 0, 0, 0]),
            giaddr: IpAddress::new([0, 0, 0, 0]),
            chaddr,
            options: vec![
                DhcpOption {
                    code: 53, // DHCP Message Type
                    length: 1,
                    data: vec![DhcpMessageType::Request as u8],
                },
                DhcpOption {
                    code: 50, // Requested IP Address
                    length: 4,
                    data: requested_ip.octets.to_vec(),
                },
            ],
        };
        packet
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        Self::from_bytes(data)
    }
}

pub fn start_client() -> Result<(), &'static str> {
    let mut interface_lock = crate::network::NETWORK_INTERFACE.lock();
    let interface = interface_lock.as_ref()
        .ok_or("Network interface not initialized")?;

    let mac_addr_obj = interface.mac_address();
    let mac_addr = mac_addr_obj.as_bytes();
    let discover_packet = DhcpPacket::new_discover(mac_addr);
    
    // Create UDP socket for DHCP
    let mut socket = Socket::new(SocketType::Dgram)?;
    socket.bind(IpAddress::new([0, 0, 0, 0]), DHCP_CLIENT_PORT)?;

    // Send DHCP discover
    let broadcast_addr = IpAddress::new([255, 255, 255, 255]);
    socket.send_to(&discover_packet.to_bytes(), broadcast_addr, DHCP_SERVER_PORT)?;

    // Implement DHCP state machine
    let mut buf = [0u8; 1500];
    let (size, _addr, _port) = socket.recv_from(&mut buf, DHCP_TIMEOUT)?;

    if let Some(offer) = DhcpPacket::from_bytes(&buf[..size]) {
        // Send DHCP request
        let mut request = discover_packet;
        request.options[0].data[0] = DhcpMessageType::Request as u8;
        request.yiaddr = offer.yiaddr;

        socket.send_to(&request.to_bytes(), broadcast_addr, DHCP_SERVER_PORT)?;

        // Wait for ACK
        let (size, _addr, _port) = socket.recv_from(&mut buf, DHCP_TIMEOUT)?;
        if let Some(ack) = DhcpPacket::from_bytes(&buf[..size]) {
            // Configure interface with received IP
            if let Some(interface) = &mut *interface_lock {
                interface.set_ip_address(ack.yiaddr);
                return Ok(());
            }
        }
    }

    Err("DHCP configuration failed")
}

pub fn start_dhcp_discovery(interface: &mut NetworkInterface) -> Result<(), &'static str> {
    let discover = DhcpPacket::new_discover(&interface.mac_address().octets());
    let discover_bytes = discover.to_bytes();
    interface.send(&discover_bytes);
    Ok(())
}

impl NetworkInterface {
    pub fn set_ip_address(&mut self, ip: IpAddress) {
        self.ip_address = ip;
    }
}

pub fn handle_dhcp_packet(udp_packet: &UdpPacket, interface: &mut NetworkInterface) -> Result<(), &'static str> {
    if let Some(dhcp_packet) = DhcpPacket::parse(&udp_packet.payload) {
        match dhcp_packet.get_message_type() {
            DhcpMessageType::Offer => {
                // Send DHCP Request
                let request = DhcpPacket::new_request(interface.mac_address().octets(), dhcp_packet.yiaddr);
                let request_bytes = request.to_bytes();
                interface.send(&request_bytes);
            }
            DhcpMessageType::Ack => {
                // Configure interface with received IP
                interface.set_ip_address(dhcp_packet.yiaddr);
            }
            _ => {}
        }
    }
    Ok(())
} 