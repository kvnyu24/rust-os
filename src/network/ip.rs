use alloc::vec::Vec;
use crate::network::IpAddress;

const IP_VERSION_4: u8 = 4;
const IP_HEADER_LEN: usize = 20;  // Without options

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IpProtocol {
    Icmp = 1,
    Tcp = 6,
    Udp = 17,
    Unknown = 255,
}

impl From<u8> for IpProtocol {
    fn from(value: u8) -> Self {
        match value {
            1 => IpProtocol::Icmp,
            6 => IpProtocol::Tcp,
            17 => IpProtocol::Udp,
            _ => IpProtocol::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct IpPacket {
    version: u8,
    header_length: u8,
    dscp: u8,
    ecn: u8,
    total_length: u16,
    identification: u16,
    flags: u8,
    fragment_offset: u16,
    ttl: u8,
    protocol: IpProtocol,
    checksum: u16,
    source: IpAddress,
    destination: IpAddress,
    payload: Vec<u8>,
}

impl IpPacket {
    pub fn new(
        source: IpAddress,
        destination: IpAddress,
        protocol: IpProtocol,
        payload: Vec<u8>,
    ) -> Self {
        let total_length = (IP_HEADER_LEN + payload.len()) as u16;

        IpPacket {
            version: IP_VERSION_4,
            header_length: (IP_HEADER_LEN / 4) as u8,
            dscp: 0,
            ecn: 0,
            total_length,
            identification: 0,  // Should be generated
            flags: 0,
            fragment_offset: 0,
            ttl: 64,  // Default TTL
            protocol,
            checksum: 0,  // Will be calculated
            source,
            destination,
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < IP_HEADER_LEN {
            return None;
        }

        let version = (data[0] >> 4) & 0xF;
        if version != IP_VERSION_4 {
            return None;
        }

        let header_length = data[0] & 0xF;
        let dscp = (data[1] >> 2) & 0x3F;
        let ecn = data[1] & 0x3;
        let total_length = u16::from_be_bytes([data[2], data[3]]);
        let identification = u16::from_be_bytes([data[4], data[5]]);
        let flags = (data[6] >> 5) & 0x7;
        let fragment_offset = u16::from_be_bytes([data[6] & 0x1F, data[7]]);
        let ttl = data[8];
        let protocol = IpProtocol::from(data[9]);
        let checksum = u16::from_be_bytes([data[10], data[11]]);

        let mut ip_bytes = [0u8; 4];
        
        // Parse source IP
        ip_bytes.copy_from_slice(&data[12..16]);
        let source = IpAddress { octets: ip_bytes };

        // Parse destination IP
        ip_bytes.copy_from_slice(&data[16..20]);
        let destination = IpAddress { octets: ip_bytes };

        // Get payload
        let payload = data[IP_HEADER_LEN..].to_vec();

        Some(IpPacket {
            version,
            header_length,
            dscp,
            ecn,
            total_length,
            identification,
            flags,
            fragment_offset,
            ttl,
            protocol,
            checksum,
            source,
            destination,
            payload,
        })
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(IP_HEADER_LEN + self.payload.len());

        // Version and Header Length
        bytes.push((self.version << 4) | self.header_length);

        // DSCP and ECN
        bytes.push((self.dscp << 2) | self.ecn);

        // Total Length
        bytes.extend_from_slice(&self.total_length.to_be_bytes());

        // Identification
        bytes.extend_from_slice(&self.identification.to_be_bytes());

        // Flags and Fragment Offset
        let flags_and_offset = ((self.flags as u16) << 13) | (self.fragment_offset & 0x1FFF);
        bytes.extend_from_slice(&flags_and_offset.to_be_bytes());

        // TTL and Protocol
        bytes.push(self.ttl);
        bytes.push(self.protocol as u8);

        // Checksum (initially 0)
        bytes.extend_from_slice(&[0, 0]);

        // Source IP
        bytes.extend_from_slice(&self.source.octets);

        // Destination IP
        bytes.extend_from_slice(&self.destination.octets);

        // Calculate checksum
        self.checksum = self.calculate_checksum(&bytes);
        bytes[10..12].copy_from_slice(&self.checksum.to_be_bytes());

        // Payload
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    fn calculate_checksum(&self, header: &[u8]) -> u16 {
        let mut sum: u32 = 0;

        // Sum up 16-bit words
        for i in (0..IP_HEADER_LEN).step_by(2) {
            sum += u16::from_be_bytes([header[i], header[i + 1]]) as u32;
        }

        // Add carried bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        // One's complement
        !sum as u16
    }

    pub fn source(&self) -> IpAddress {
        self.source
    }

    pub fn destination(&self) -> IpAddress {
        self.destination
    }

    pub fn protocol(&self) -> IpProtocol {
        self.protocol
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}