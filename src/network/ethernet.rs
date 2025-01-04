use alloc::vec::Vec;
use crate::network::MacAddress;

const ETHERNET_HEADER_LEN: usize = 14;
const MIN_FRAME_SIZE: usize = 64;
const MAX_FRAME_SIZE: usize = 1518;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EtherType {
    Ipv4 = 0x0800,
    Arp = 0x0806,
    Unknown = 0xFFFF,
}

impl From<u16> for EtherType {
    fn from(value: u16) -> Self {
        match value {
            0x0800 => EtherType::Ipv4,
            0x0806 => EtherType::Arp,
            _ => EtherType::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct EthernetFrame {
    destination: MacAddress,
    source: MacAddress,
    ethertype: EtherType,
    payload: Vec<u8>,
}

impl EthernetFrame {
    pub fn new(destination: MacAddress, source: MacAddress, ethertype: EtherType, payload: Vec<u8>) -> Self {
        EthernetFrame {
            destination,
            source,
            ethertype,
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < ETHERNET_HEADER_LEN {
            return None;
        }

        let mut mac_bytes = [0u8; 6];

        // Parse destination MAC
        mac_bytes.copy_from_slice(&data[0..6]);
        let destination = MacAddress::new(mac_bytes);

        // Parse source MAC
        mac_bytes.copy_from_slice(&data[6..12]);
        let source = MacAddress::new(mac_bytes);

        // Parse EtherType
        let ethertype = u16::from_be_bytes([data[12], data[13]]);
        let ethertype = EtherType::from(ethertype);

        // Get payload
        let payload = data[ETHERNET_HEADER_LEN..].to_vec();

        Some(EthernetFrame {
            destination,
            source,
            ethertype,
            payload,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(ETHERNET_HEADER_LEN + self.payload.len());

        // Add destination MAC
        bytes.extend_from_slice(&self.destination.octets());

        // Add source MAC
        bytes.extend_from_slice(&self.source.octets());

        // Add EtherType
        bytes.extend_from_slice(&(self.ethertype as u16).to_be_bytes());

        // Add payload
        bytes.extend_from_slice(&self.payload);

        // Pad if necessary
        if bytes.len() < MIN_FRAME_SIZE {
            bytes.resize(MIN_FRAME_SIZE, 0);
        }

        bytes
    }

    pub fn destination(&self) -> &MacAddress {
        &self.destination
    }

    pub fn source(&self) -> &MacAddress {
        &self.source
    }

    pub fn ethertype(&self) -> EtherType {
        self.ethertype
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}