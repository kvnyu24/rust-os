use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::Mutex;
use lazy_static::lazy_static;
use core::sync::atomic::AtomicU32;
use crate::network::{IpAddress, tcp, udp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,  // TCP
    Dgram,   // UDP
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed,
    Listening,
    Connected,
    Error,
}

#[derive(Debug)]
pub struct Socket {
    socket_type: SocketType,
    state: SocketState,
    local_addr: IpAddress,
    local_port: u16,
    remote_addr: Option<IpAddress>,
    remote_port: Option<u16>,
    receive_buffer: Vec<u8>,
    tcp_connection: Option<tcp::TcpConnection>,
}

type SocketId = u32;
static NEXT_SOCKET_ID: AtomicU32 = AtomicU32::new(1);

lazy_static! {
    static ref SOCKETS: Mutex<BTreeMap<SocketId, Arc<Mutex<Socket>>>> = Mutex::new(BTreeMap::new());
}

impl Socket {
    pub fn new(socket_type: SocketType) -> Self {
        Socket {
            socket_type,
            state: SocketState::Closed,
            local_addr: IpAddress::new([0, 0, 0, 0]),
            local_port: 0,
            remote_addr: None,
            remote_port: None,
            receive_buffer: Vec::new(),
            tcp_connection: None,
        }
    }

    pub fn bind(&mut self, addr: IpAddress, port: u16) -> Result<(), &'static str> {
        if self.state != SocketState::Closed {
            return Err("Socket already bound or connected");
        }

        // Check if port is already in use
        for socket in SOCKETS.lock().values() {
            let socket = socket.lock();
            if socket.local_port == port && socket.local_addr == addr {
                return Err("Address already in use");
            }
        }

        self.local_addr = addr;
        self.local_port = port;

        match self.socket_type {
            SocketType::Dgram => {
                // Register UDP callback
                udp::bind(port, Box::new(move |data, src_ip, src_port| {
                    if let Some(socket) = find_socket_by_port(port) {
                        let mut socket = socket.lock();
                        socket.handle_udp_data(data, src_ip, src_port);
                    }
                }))?;
            }
            SocketType::Stream => {
                // Create TCP connection
                self.tcp_connection = Some(tcp::TcpConnection::new(addr, port));
            }
        }

        Ok(())
    }

    pub fn listen(&mut self) -> Result<(), &'static str> {
        if self.socket_type != SocketType::Stream {
            return Err("Only TCP sockets can listen");
        }

        if self.state != SocketState::Closed {
            return Err("Socket already in use");
        }

        if let Some(conn) = &mut self.tcp_connection {
            conn.listen()?;
            self.state = SocketState::Listening;
            Ok(())
        } else {
            Err("Socket not bound")
        }
    }

    pub fn connect(&mut self, addr: IpAddress, port: u16) -> Result<(), &'static str> {
        if self.state != SocketState::Closed {
            return Err("Socket already connected");
        }

        match self.socket_type {
            SocketType::Stream => {
                if let Some(conn) = &mut self.tcp_connection {
                    conn.connect(addr, port)?;
                    self.remote_addr = Some(addr);
                    self.remote_port = Some(port);
                    self.state = SocketState::Connected;
                    Ok(())
                } else {
                    Err("Socket not bound")
                }
            }
            SocketType::Dgram => {
                // UDP doesn't need connection, just store the remote address
                self.remote_addr = Some(addr);
                self.remote_port = Some(port);
                self.state = SocketState::Connected;
                Ok(())
            }
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<usize, &'static str> {
        match self.socket_type {
            SocketType::Stream => {
                if self.state != SocketState::Connected {
                    return Err("Socket not connected");
                }

                if let Some(conn) = &mut self.tcp_connection {
                    conn.send(data)?;
                    Ok(data.len())
                } else {
                    Err("Socket not initialized")
                }
            }
            SocketType::Dgram => {
                if let Some(addr) = self.remote_addr {
                    if let Some(port) = self.remote_port {
                        udp::send(self.local_port, addr, port, data)?;
                        Ok(data.len())
                    } else {
                        Err("Remote port not set")
                    }
                } else {
                    Err("Remote address not set")
                }
            }
        }
    }

    pub fn sendto(&mut self, data: &[u8], addr: IpAddress, port: u16) -> Result<usize, &'static str> {
        if self.socket_type != SocketType::Dgram {
            return Err("Operation not supported for TCP sockets");
        }

        udp::send(self.local_port, addr, port, data)?;
        Ok(data.len())
    }

    pub fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, &'static str> {
        if self.receive_buffer.is_empty() {
            return Ok(0);
        }

        let len = core::cmp::min(buffer.len(), self.receive_buffer.len());
        buffer[..len].copy_from_slice(&self.receive_buffer[..len]);
        self.receive_buffer.drain(..len);
        Ok(len)
    }

    fn handle_udp_data(&mut self, data: &[u8], src_ip: IpAddress, src_port: u16) {
        if self.remote_addr.is_none() || self.remote_addr == Some(src_ip) {
            self.receive_buffer.extend_from_slice(data);
        }
    }
}

pub fn socket(socket_type: SocketType) -> Result<SocketId, &'static str> {
    let socket = Arc::new(Mutex::new(Socket::new(socket_type)));
    let id = NEXT_SOCKET_ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    SOCKETS.lock().insert(id, socket);
    Ok(id)
}

pub fn bind(socket_id: SocketId, addr: IpAddress, port: u16) -> Result<(), &'static str> {
    if let Some(socket) = SOCKETS.lock().get(&socket_id) {
        socket.lock().bind(addr, port)
    } else {
        Err("Invalid socket")
    }
}

pub fn listen(socket_id: SocketId) -> Result<(), &'static str> {
    if let Some(socket) = SOCKETS.lock().get(&socket_id) {
        socket.lock().listen()
    } else {
        Err("Invalid socket")
    }
}

pub fn connect(socket_id: SocketId, addr: IpAddress, port: u16) -> Result<(), &'static str> {
    if let Some(socket) = SOCKETS.lock().get(&socket_id) {
        socket.lock().connect(addr, port)
    } else {
        Err("Invalid socket")
    }
}

pub fn send(socket_id: SocketId, data: &[u8]) -> Result<usize, &'static str> {
    if let Some(socket) = SOCKETS.lock().get(&socket_id) {
        socket.lock().send(data)
    } else {
        Err("Invalid socket")
    }
}

pub fn sendto(socket_id: SocketId, data: &[u8], addr: IpAddress, port: u16) -> Result<usize, &'static str> {
    if let Some(socket) = SOCKETS.lock().get(&socket_id) {
        socket.lock().sendto(data, addr, port)
    } else {
        Err("Invalid socket")
    }
}

pub fn receive(socket_id: SocketId, buffer: &mut [u8]) -> Result<usize, &'static str> {
    if let Some(socket) = SOCKETS.lock().get(&socket_id) {
        socket.lock().receive(buffer)
    } else {
        Err("Invalid socket")
    }
}

pub fn close(socket_id: SocketId) -> Result<(), &'static str> {
    SOCKETS.lock().remove(&socket_id);
    Ok(())
}

fn find_socket_by_port(port: u16) -> Option<Arc<Mutex<Socket>>> {
    for socket in SOCKETS.lock().values() {
        let socket_ref = socket.lock();
        if socket_ref.local_port == port {
            return Some(Arc::clone(socket));
        }
    }
    None
} 