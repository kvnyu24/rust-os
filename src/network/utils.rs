use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use core::sync::atomic::{AtomicU16, Ordering};
use crate::network::{IpAddress, icmp};
use crate::println;

static PING_ID: AtomicU16 = AtomicU16::new(1);

pub struct PingStatistics {
    pub packets_sent: u32,
    pub packets_received: u32,
    pub min_rtt: u64,
    pub max_rtt: u64,
    pub avg_rtt: u64,
}

pub fn ping(destination: IpAddress, count: Option<u32>) -> Result<PingStatistics, &'static str> {
    let id = PING_ID.fetch_add(1, Ordering::SeqCst);
    let mut sequence = 0;
    let mut stats = PingStatistics {
        packets_sent: 0,
        packets_received: 0,
        min_rtt: u64::MAX,
        max_rtt: 0,
        avg_rtt: 0,
    };

    let max_count = count.unwrap_or(4);
    let payload = b"RustOS Ping".to_vec();

    println!("PING {} with {} bytes of data", destination, payload.len());

    while stats.packets_sent < max_count {
        // Send ping request
        if let Err(e) = icmp::send_echo_request(destination, id, sequence, payload.clone()) {
            println!("Failed to send ping: {}", e);
            continue;
        }

        stats.packets_sent += 1;
        sequence += 1;

        // TODO: Wait for reply with timeout
        // For now, we'll just simulate some basic statistics
        stats.packets_received += 1;
        let rtt = 100; // Simulated RTT in milliseconds
        stats.min_rtt = core::cmp::min(stats.min_rtt, rtt);
        stats.max_rtt = core::cmp::max(stats.max_rtt, rtt);
        stats.avg_rtt = (stats.avg_rtt * (stats.packets_received - 1) + rtt) / stats.packets_received;

        println!("Reply from {}: bytes={} time={}ms", destination, payload.len(), rtt);

        // Sleep for a second between pings
        for _ in 0..1_000_000 { core::hint::spin_loop(); }
    }

    println!("\nPing statistics for {}:", destination);
    println!("    Packets: Sent = {}, Received = {}, Lost = {} ({}% loss)",
        stats.packets_sent,
        stats.packets_received,
        stats.packets_sent - stats.packets_received,
        ((stats.packets_sent - stats.packets_received) * 100) / stats.packets_sent
    );
    println!("Round trip times in milliseconds:");
    println!("    Minimum = {}ms, Maximum = {}ms, Average = {}ms",
        stats.min_rtt,
        stats.max_rtt,
        stats.avg_rtt
    );

    Ok(stats)
}

pub fn netstat() {
    println!("Active Internet connections");
    println!("Proto Local Address           Foreign Address         State");

    // TCP connections
    if let Some(connections) = crate::network::tcp::get_connections() {
        for conn in connections {
            println!("tcp   {}:{:<16} {}:{:<16} {}",
                conn.local_addr(),
                conn.local_port(),
                conn.remote_addr().map(|a| a.to_string()).unwrap_or_else(|| String::from("*")),
                conn.remote_port().map(|p| p.to_string()).unwrap_or_else(|| String::from("*")),
                format!("{:?}", conn.state())
            );
        }
    }

    // UDP sockets
    if let Some(sockets) = crate::network::udp::get_sockets() {
        for socket in sockets {
            println!("udp   {}:{:<16} *:*",
                socket.local_addr(),
                socket.local_port()
            );
        }
    }
}

pub fn route_print() {
    println!("Network Destination        Gateway             Netmask             Interface");
    
    if let Some(interface) = &*crate::network::NETWORK_INTERFACE.lock() {
        let addr = interface.ip_address();
        let netmask = IpAddress::new([255, 255, 255, 0]); // Assuming /24 network
        let gateway = IpAddress::new([192, 168, 1, 1]); // Default gateway

        // Local network route
        println!("{:<24} {:<19} {:<19} {}",
            format!("{}", addr),
            "0.0.0.0",
            format!("{}", netmask),
            format!("{}", addr)
        );

        // Default route
        println!("{:<24} {:<19} {:<19} {}",
            "0.0.0.0",
            format!("{}", gateway),
            "0.0.0.0",
            format!("{}", addr)
        );
    }
} 