use crate::network::{IpAddress, dns, utils};
use crate::println;

pub fn run_network_tests() {
    println!("\nRunning network tests...\n");

    // Test DNS resolution
    match dns::resolve_hostname("example.com") {
        Ok(ip) => println!("Resolved example.com to {}", ip),
        Err(e) => println!("Failed to resolve example.com: {}", e),
    }

    // Test ping
    let destination = IpAddress::new([8, 8, 8, 8]); // Google DNS
    match utils::ping(destination, Some(3)) {
        Ok(stats) => {
            println!("\nPing statistics:");
            println!("  Packets: sent = {}, received = {}, lost = {}",
                stats.packets_sent,
                stats.packets_received,
                stats.packets_sent - stats.packets_received
            );
            println!("  RTT: min = {}ms, max = {}ms, avg = {}ms",
                stats.min_rtt,
                stats.max_rtt,
                stats.avg_rtt
            );
        }
        Err(e) => println!("Ping failed: {}", e),
    }

    // Display network interfaces and routing
    println!("\nNetwork interfaces and routing:");
    utils::route_print();

    // Display active connections
    println!("\nActive network connections:");
    utils::netstat();
} 