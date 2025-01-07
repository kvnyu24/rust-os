use alloc::format;
use alloc::vec::Vec;
use core::time::Duration;
use core::sync::atomic::{AtomicU16, Ordering};
use x86_64::instructions::port::Port;
use crate::network::{IpAddress, icmp::{IcmpPacket, IcmpType}};
use crate::println;
use crate::network::prelude::*;
use crate::network::driver::NETWORK_DRIVER;
use crate::network::socket::{Socket, SOCKETS};

static PING_ID: AtomicU16 = AtomicU16::new(1);

#[derive(Debug)]
pub struct PingStatistics {
    pub packets_sent: u32,
    pub packets_received: u32,
    pub min_rtt: u64,
    pub max_rtt: u64,
    pub avg_rtt: u64,
}

impl PingStatistics {
    pub fn new() -> Self {
        PingStatistics {
            packets_sent: 0,
            packets_received: 0,
            min_rtt: u64::MAX,
            max_rtt: 0,
            avg_rtt: 0,
        }
    }

    pub fn update(&mut self, rtt: u64) {
        self.packets_received += 1;
        self.min_rtt = self.min_rtt.min(rtt);
        self.max_rtt = self.max_rtt.max(rtt);
        
        // Update average RTT using weighted average
        self.avg_rtt = if self.packets_received == 1 {
            rtt
        } else {
            ((self.avg_rtt * (self.packets_received - 1) as u64) + rtt) / self.packets_received as u64
        };
    }
}

pub fn get_timestamp() -> u64 {
    unsafe {
        let mut port: Port<u8> = Port::new(0x70);
        port.write(0u8);
        let mut port: Port<u8> = Port::new(0x71);
        port.read() as u64
    }
}

pub fn sleep(duration: Duration) {
    let start = get_timestamp();
    while get_timestamp() - start < duration.as_millis() as u64 {
        core::hint::spin_loop();
    }
}

pub fn ping(dest_ip: IpAddress, count: u32) -> Result<PingStatistics, &'static str> {
    let mut stats = PingStatistics::new();
    let id = PING_ID.fetch_add(1, Ordering::SeqCst);
    
    for sequence in 1..=count {
        stats.packets_sent += 1;

        // Create ICMP echo request with timestamp
        let timestamp = get_timestamp();
            
        let mut packet = IcmpPacket::new_echo_request(
            id,
            sequence as u16,
            timestamp.to_be_bytes().to_vec(),
        );

        if let Some(driver) = &mut *NETWORK_DRIVER.lock() {
            driver.send(&packet.to_bytes())?;

            // Wait for reply with timeout
            let mut attempts = 0;
            const MAX_ATTEMPTS: u64 = 3;
            const TIMEOUT_MS: u64 = 1000;

            while attempts < MAX_ATTEMPTS {
                let mut buffer = vec![0; 1500]; // Standard MTU size
                if let Some(received_data) = driver.receive() {
                    if received_data.len() > 0 {
                        if let Some(reply) = IcmpPacket::parse(&received_data) {
                            if reply.get_type() == IcmpType::EchoReply && reply.get_identifier() == id {
                                let now = get_timestamp();
                                let rtt = now.saturating_sub(timestamp);
                                stats.update(rtt);
                                break;
                            }
                        }
                    }
                }
                attempts += 1;
                sleep(Duration::from_millis(TIMEOUT_MS / MAX_ATTEMPTS));
            }
        }
    }

    Ok(stats)
}

pub fn netstat() {
    let sockets = SOCKETS.lock();
    for (id, socket) in sockets.iter() {
        if let Some(guard) = socket.try_lock() {
            println!("Socket {}: {:?}", id, guard.state);
        }
    }
}

pub fn route_print() {
    let sockets = SOCKETS.lock();
    for (id, socket) in sockets.iter() {
        if let Some(guard) = socket.try_lock() {
            println!("Socket {}: {}", id, guard.local_addr());
        }
    }
}

pub fn check_sockets() -> Result<(), &'static str> {
    let sockets = SOCKETS.lock();
    for socket in sockets.values() {
        // Process socket
    }
    Ok(())
}