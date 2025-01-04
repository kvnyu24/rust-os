use alloc::vec::Vec;
use crate::network::prelude::*;
use crate::network::socket;
use crate::network::utils;
use crate::network::IpAddress;
use core::time::Duration;

pub fn run_network_tests() -> Result<(), &'static str> {
    // Test ping
    let test_ips = [
        IpAddress::new([8, 8, 8, 8]),      // Google DNS
        IpAddress::new([1, 1, 1, 1]),      // Cloudflare DNS
    ];

    for ip in &test_ips {
        match utils::ping(*ip, 3) {
            Ok(stats) => {
                println!("✓ Ping test passed for {}", ip);
                println!("  Packets: sent={}, received={}", stats.packets_sent, stats.packets_received);
                println!("  RTT: min={}ms, avg={}ms, max={}ms", stats.min_rtt, stats.avg_rtt, stats.max_rtt);
            }
            Err(e) => println!("✗ Ping test failed for {}: {}", ip, e),
        }
    }

    // Test TCP
    let socket_id = socket::socket(socket::SocketType::Stream)?;
    socket::bind(socket_id, IpAddress::new([0, 0, 0, 0]), 0)?;

    match socket::connect(socket_id, IpAddress::new([93, 184, 216, 34]), 80) {
        Ok(_) => {
            println!("✓ TCP connection test passed");
            let data = b"GET / HTTP/1.0\r\n\r\n";
            match socket::send(socket_id, data) {
                Ok(_) => {
                    let mut buffer = Vec::with_capacity(1024);
                    match socket::receive(socket_id, &mut buffer) {
                        Ok((size, addr, port)) => println!("✓ Received {:?} bytes from {}:{}", size, addr, port),
                        Err(e) => println!("✗ TCP receive failed: {}", e),
                    }
                }
                Err(e) => println!("✗ TCP send failed: {}", e),
            }
        }
        Err(e) => println!("✗ TCP connection test failed: {}", e),
    }

    socket::close(socket_id)?;

    // Test UDP
    let socket_id = socket::socket(socket::SocketType::Dgram)?;
    socket::bind(socket_id, IpAddress::new([0, 0, 0, 0]), 0)?;

    let dns_addr = IpAddress::new([8, 8, 8, 8]);
    let data = b"\x00\x01\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x03www\x06google\x03com\x00\x00\x01\x00\x01";

    match socket::send_to(socket_id, data, dns_addr, 53) {
        Ok(_) => {
            let mut buffer = Vec::with_capacity(1024);
            match socket::receive(socket_id, &mut buffer) {
                Ok((size, addr, port)) => println!("✓ Received {:?} bytes from {}:{}", size, addr, port),
                Err(e) => println!("✗ UDP receive failed: {}", e),
            }
        }
        Err(e) => println!("✗ UDP send failed: {}", e),
    }

    socket::close(socket_id)?;

    Ok(())
} 