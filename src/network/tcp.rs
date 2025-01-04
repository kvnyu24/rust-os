use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use core::fmt;
use crate::network::{IpAddress, NETWORK_DRIVER};
use spin::Mutex;

/// Length of TCP header without options
const TCP_HEADER_LEN: usize = 20;

/// Represents the possible states of a TCP connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// TCP control flags used in the TCP header
#[derive(Debug, Clone, Copy)]
pub struct TcpFlags {
    pub fin: bool,  // Finish flag
    pub syn: bool,  // Synchronize flag
    pub rst: bool,  // Reset flag 
    pub psh: bool,  // Push flag
    pub ack: bool,  // Acknowledgment flag
    pub urg: bool,  // Urgent flag
}

impl TcpFlags {
    /// Creates a new TcpFlags struct with all flags set to false
    pub fn new() -> Self {
        TcpFlags {
            fin: false,
            syn: false,
            rst: false,
            psh: false,
            ack: false,
            urg: false,
        }
    }

    /// Creates TcpFlags from a byte representation
    pub fn from_byte(byte: u8) -> Self {
        TcpFlags {
            fin: (byte & 0x01) != 0,
            syn: (byte & 0x02) != 0,
            rst: (byte & 0x04) != 0,
            psh: (byte & 0x08) != 0,
            ack: (byte & 0x10) != 0,
            urg: (byte & 0x20) != 0,
        }
    }

    /// Converts TcpFlags to a byte representation
    pub fn to_byte(&self) -> u8 {
        let mut byte = 0;
        if self.fin { byte |= 0x01; }
        if self.syn { byte |= 0x02; }
        if self.rst { byte |= 0x04; }
        if self.psh { byte |= 0x08; }
        if self.ack { byte |= 0x10; }
        if self.urg { byte |= 0x20; }
        byte
    }
}

/// Represents a TCP segment with header fields and payload
#[derive(Debug)]
pub struct TcpSegment {
    source_port: u16,
    destination_port: u16,
    sequence_number: u32,
    acknowledgment_number: u32,
    data_offset: u8,
    flags: TcpFlags,
    window_size: u16,
    checksum: u16,
    urgent_pointer: u16,
    payload: Vec<u8>,
}

impl TcpSegment {
    /// Creates a new TCP segment with the given parameters
    pub fn new(
        source_port: u16,
        destination_port: u16,
        sequence_number: u32,
        acknowledgment_number: u32,
        flags: TcpFlags,
        window_size: u16,
        payload: Vec<u8>,
    ) -> Self {
        TcpSegment {
            source_port,
            destination_port,
            sequence_number,
            acknowledgment_number,
            data_offset: (TCP_HEADER_LEN / 4) as u8,
            flags,
            window_size,
            checksum: 0,  // Will be calculated later
            urgent_pointer: 0,
            payload,
        }
    }

    /// Parses a byte slice into a TCP segment
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < TCP_HEADER_LEN {
            return None;
        }

        let source_port = u16::from_be_bytes([data[0], data[1]]);
        let destination_port = u16::from_be_bytes([data[2], data[3]]);
        let sequence_number = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let acknowledgment_number = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let data_offset = (data[12] >> 4) & 0xF;
        let flags = TcpFlags::from_byte(data[13]);
        let window_size = u16::from_be_bytes([data[14], data[15]]);
        let checksum = u16::from_be_bytes([data[16], data[17]]);
        let urgent_pointer = u16::from_be_bytes([data[18], data[19]]);

        let header_len = (data_offset as usize) * 4;
        let payload = if data.len() > header_len {
            data[header_len..].to_vec()
        } else {
            Vec::new()
        };

        Some(TcpSegment {
            source_port,
            destination_port,
            sequence_number,
            acknowledgment_number,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_pointer,
            payload,
        })
    }

    /// Converts the TCP segment to a byte vector
    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(TCP_HEADER_LEN + self.payload.len());

        // Source port
        bytes.extend_from_slice(&self.source_port.to_be_bytes());

        // Destination port
        bytes.extend_from_slice(&self.destination_port.to_be_bytes());

        // Sequence number
        bytes.extend_from_slice(&self.sequence_number.to_be_bytes());

        // Acknowledgment number
        bytes.extend_from_slice(&self.acknowledgment_number.to_be_bytes());

        // Data offset and reserved bits
        bytes.push((self.data_offset << 4) & 0xF0);

        // Flags
        bytes.push(self.flags.to_byte());

        // Window size
        bytes.extend_from_slice(&self.window_size.to_be_bytes());

        // Checksum (initially 0)
        bytes.extend_from_slice(&[0, 0]);

        // Urgent pointer
        bytes.extend_from_slice(&self.urgent_pointer.to_be_bytes());

        // Payload
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    /// Calculates TCP checksum including pseudo-header
    pub fn calculate_checksum(&mut self, source_ip: IpAddress, dest_ip: IpAddress) {
        let mut sum: u32 = 0;

        // Add source IP
        for byte in source_ip.octets.chunks(2) {
            sum += u16::from_be_bytes([byte[0], byte[1]]) as u32;
        }

        // Add destination IP
        for byte in dest_ip.octets.chunks(2) {
            sum += u16::from_be_bytes([byte[0], byte[1]]) as u32;
        }

        // Add protocol number (6 for TCP) and TCP length
        sum += 6u32;
        sum += (TCP_HEADER_LEN + self.payload.len()) as u32;

        // Add TCP header and data
        let segment_bytes = self.to_bytes();
        for chunk in segment_bytes.chunks(2) {
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

/// Represents a TCP connection with associated state and buffers
#[derive(Debug)]
pub struct TcpConnection {
    state: TcpState,
    local_addr: IpAddress,
    remote_addr: IpAddress,
    local_port: u16,
    remote_port: u16,
    sequence_number: u32,
    acknowledgment_number: u32,
    window_size: u16,
    receive_buffer: Vec<u8>,
}

impl TcpConnection {
    /// Creates a new TCP connection bound to the given local address and port
    pub fn new(local_addr: IpAddress, local_port: u16) -> Self {
        TcpConnection {
            state: TcpState::Closed,
            local_addr,
            remote_addr: IpAddress::new([0, 0, 0, 0]),
            local_port,
            remote_port: 0,
            sequence_number: 0,
            acknowledgment_number: 0,
            window_size: 8192,
            receive_buffer: Vec::new(),
        }
    }

    /// Initiates a TCP connection to the specified remote endpoint
    pub fn connect(&mut self, remote_addr: IpAddress, remote_port: u16) -> Result<(), &'static str> {
        if self.state != TcpState::Closed {
            return Err("Connection already exists");
        }

        self.remote_addr = remote_addr;
        self.remote_port = remote_port;
        self.sequence_number = 0;  // TODO: Should be random for security

        // Send SYN
        let mut flags = TcpFlags::new();
        flags.syn = true;

        let mut segment = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.sequence_number,
            0,
            flags,
            self.window_size,
            Vec::new(),
        );

        segment.calculate_checksum(self.local_addr, self.remote_addr);
        if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
            driver.send(&segment.to_bytes())?;
        }

        self.state = TcpState::SynSent;
        self.sequence_number += 1;

        Ok(())
    }

    /// Handles an incoming TCP segment based on current connection state
    pub fn handle_segment(&mut self, segment: TcpSegment) {
        match self.state {
            TcpState::Listen => {
                if segment.flags.syn {
                    self.handle_syn_received(segment);
                }
            }
            TcpState::SynSent => {
                if segment.flags.syn && segment.flags.ack {
                    self.handle_syn_ack_received(segment);
                }
            }
            TcpState::Established => {
                self.handle_established(segment);
            }
            _ => {}
        }
    }

    /// Handles incoming SYN segment in Listen state
    fn handle_syn_received(&mut self, segment: TcpSegment) {
        self.remote_addr = IpAddress::new([0, 0, 0, 0]);  // TODO: Get from IP header
        self.remote_port = segment.source_port;
        self.acknowledgment_number = segment.sequence_number + 1;

        // Send SYN-ACK
        let mut flags = TcpFlags::new();
        flags.syn = true;
        flags.ack = true;

        let mut response = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.sequence_number,
            self.acknowledgment_number,
            flags,
            self.window_size,
            Vec::new(),
        );

        response.calculate_checksum(self.local_addr, self.remote_addr);
        if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
            let _ = driver.send(&response.to_bytes());
        }

        self.state = TcpState::SynReceived;
        self.sequence_number += 1;
    }

    /// Handles SYN-ACK segment in SynSent state
    fn handle_syn_ack_received(&mut self, segment: TcpSegment) {
        if segment.acknowledgment_number == self.sequence_number {
            self.acknowledgment_number = segment.sequence_number + 1;

            // Send ACK
            let mut flags = TcpFlags::new();
            flags.ack = true;

            let mut response = TcpSegment::new(
                self.local_port,
                self.remote_port,
                self.sequence_number,
                self.acknowledgment_number,
                flags,
                self.window_size,
                Vec::new(),
            );

            response.calculate_checksum(self.local_addr, self.remote_addr);
            if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
                let _ = driver.send(&response.to_bytes());
            }

            self.state = TcpState::Established;
        }
    }

    /// Handles segments in Established state
    fn handle_established(&mut self, segment: TcpSegment) {
        if !segment.payload.is_empty() {
            // Process received data
            self.receive_buffer.extend_from_slice(&segment.payload);
            self.acknowledgment_number += segment.payload.len() as u32;

            // Send ACK
            let mut flags = TcpFlags::new();
            flags.ack = true;

            let mut response = TcpSegment::new(
                self.local_port,
                self.remote_port,
                self.sequence_number,
                self.acknowledgment_number,
                flags,
                self.window_size,
                Vec::new(),
            );

            response.calculate_checksum(self.local_addr, self.remote_addr);
            if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
                let _ = driver.send(&response.to_bytes());
            }
        }
    }

    /// Sends data over the established TCP connection
    pub fn send(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Connection not established");
        }

        let mut flags = TcpFlags::new();
        flags.psh = true;
        flags.ack = true;

        let mut segment = TcpSegment::new(
            self.local_port,
            self.remote_port,
            self.sequence_number,
            self.acknowledgment_number,
            flags,
            self.window_size,
            data.to_vec(),
        );

        segment.calculate_checksum(self.local_addr, self.remote_addr);
        if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
            driver.send(&segment.to_bytes())?;
        }

        self.sequence_number += data.len() as u32;
        Ok(())
    }

    pub fn start_listen(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Closed {
            return Err("Connection not in closed state");
        }
        self.state = TcpState::Listen;
        Ok(())
    }
}

/// Handles an incoming TCP segment
pub fn handle_tcp_segment(segment: TcpSegment, source_ip: IpAddress, dest_ip: IpAddress) {
    // TODO: Implement TCP connection handling logic
}