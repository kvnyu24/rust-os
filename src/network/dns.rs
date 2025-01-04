use alloc::vec::Vec;
use alloc::string::{String, ToString};
use core::convert::TryInto;
use crate::network::{IpAddress, udp};

const DNS_PORT: u16 = 53;

#[derive(Debug)]
pub struct DnsHeader {
    id: u16,
    flags: u16,
    questions: u16,
    answers: u16,
    authority: u16,
    additional: u16,
}

#[derive(Debug)]
pub struct DnsQuestion {
    name: String,
    qtype: u16,
    qclass: u16,
}

#[derive(Debug)]
pub struct DnsAnswer {
    name: String,
    atype: u16,
    aclass: u16,
    ttl: u32,
    rdlength: u16,
    rdata: Vec<u8>,
}

pub struct DnsResolver {
    server: IpAddress,
    next_id: u16,
}

impl DnsResolver {
    pub fn new(server: IpAddress) -> Self {
        Self {
            server,
            next_id: 0,
        }
    }

    pub fn resolve(&mut self, hostname: &str) -> Result<IpAddress, &'static str> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        let mut packet = Vec::new();
        
        // Build DNS header
        packet.extend_from_slice(&id.to_be_bytes());
        packet.extend_from_slice(&0x0100u16.to_be_bytes()); // Standard query
        packet.extend_from_slice(&1u16.to_be_bytes()); // One question
        packet.extend_from_slice(&0u16.to_be_bytes()); // No answers
        packet.extend_from_slice(&0u16.to_be_bytes()); // No authority
        packet.extend_from_slice(&0u16.to_be_bytes()); // No additional

        // Encode hostname
        for label in hostname.split('.') {
            packet.push(label.len() as u8);
            packet.extend_from_slice(label.as_bytes());
        }
        packet.push(0); // Terminating null label

        // Query type (A record) and class (IN)
        packet.extend_from_slice(&1u16.to_be_bytes());
        packet.extend_from_slice(&1u16.to_be_bytes());

        // Send query
        let socket = udp::Socket::new()?;
        socket.send_to(&packet, self.server, DNS_PORT)?;

        // Receive response
        let mut response = vec![0; 512];
        let (size, _) = socket.recv_from(&mut response)?;
        response.truncate(size);

        // Parse response
        if size < 12 {
            return Err("Response too short");
        }

        let header = DnsHeader {
            id: u16::from_be_bytes(response[0..2].try_into().unwrap()),
            flags: u16::from_be_bytes(response[2..4].try_into().unwrap()),
            questions: u16::from_be_bytes(response[4..6].try_into().unwrap()),
            answers: u16::from_be_bytes(response[6..8].try_into().unwrap()),
            authority: u16::from_be_bytes(response[8..10].try_into().unwrap()),
            additional: u16::from_be_bytes(response[10..12].try_into().unwrap()),
        };

        if header.id != id {
            return Err("Response ID mismatch");
        }

        if (header.flags & 0x8000) == 0 {
            return Err("Not a response");
        }

        if (header.flags & 0x000F) != 0 {
            return Err("DNS error in response");
        }

        if header.answers == 0 {
            return Err("No answers in response");
        }

        // Skip questions section
        let mut pos = 12;
        for _ in 0..header.questions {
            while pos < size {
                let len = response[pos] as usize;
                if len == 0 {
                    pos += 1;
                    break;
                }
                pos += len + 1;
            }
            pos += 4; // Skip qtype and qclass
        }

        // Parse first answer
        while pos < size {
            let len = response[pos] as usize;
            if len == 0 {
                pos += 1;
                break;
            }
            pos += len + 1;
        }

        if pos + 10 > size {
            return Err("Response truncated");
        }

        let atype = u16::from_be_bytes(response[pos..pos+2].try_into().unwrap());
        let aclass = u16::from_be_bytes(response[pos+2..pos+4].try_into().unwrap());
        let ttl = u32::from_be_bytes(response[pos+4..pos+8].try_into().unwrap());
        let rdlength = u16::from_be_bytes(response[pos+8..pos+10].try_into().unwrap());

        pos += 10;

        if atype != 1 || aclass != 1 {
            return Err("Not an A record");
        }

        if rdlength != 4 {
            return Err("Invalid A record length");
        }

        if pos + 4 > size {
            return Err("Response truncated");
        }

        Ok(IpAddress::new([
            response[pos],
            response[pos+1],
            response[pos+2],
            response[pos+3],
        ]))
    }
}

pub fn resolve_hostname(hostname: &str) -> Result<IpAddress, &'static str> {
    let mut resolver = DnsResolver::new(IpAddress::new([8, 8, 8, 8])); // Google DNS
    resolver.resolve(hostname)
} 