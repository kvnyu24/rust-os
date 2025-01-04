pub use alloc::boxed::Box;
pub use alloc::string::{String, ToString};
pub use alloc::vec;
pub use alloc::vec::Vec;
pub use core::fmt::{self, Display, Formatter};
pub use lazy_static::lazy_static;
pub use spin::Mutex;

// Re-export common types from our own modules
pub use crate::network::{
    arp::ArpPacket,
    dhcp::DhcpPacket,
    dns::DnsPacket,
    ethernet::EthernetFrame,
    icmp::IcmpPacket,
    ip::IpPacket,
    socket::Socket,
    tcp::TcpPacket,
    udp::UdpPacket,
    MacAddress, IpAddress, PortNumber,
};

// Common result type for network operations
pub type Result<T> = core::result::Result<T, NetworkError>;

#[derive(Debug)]
pub enum NetworkError {
    BufferTooSmall,
    InvalidPacket,
    SocketError,
    AddressInUse,
    ConnectionRefused,
    NotConnected,
    Timeout,
    Other(&'static str),
} 