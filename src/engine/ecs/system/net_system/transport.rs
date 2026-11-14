//! Nonblocking UDP polled from the frame loop: no worker thread, bounded
//! receive work per poll, oversize datagrams discarded rather than truncated.
use super::protocol::{MAX_DATAGRAM, Packet};
use std::{
    io,
    net::{SocketAddr, UdpSocket},
};

/// Datagrams drained per poll; the rest wait for the next frame.
const MAX_RECV_PER_POLL: usize = 256;

#[derive(Debug, Default, Clone, Copy)]
pub struct TransportStats {
    pub sent_packets: u64,
    pub sent_bytes: u64,
    pub received_packets: u64,
    pub received_bytes: u64,
    pub oversize_drops: u64,
    pub decode_drops: u64,
    pub send_errors: u64,
}

#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    // Larger than MAX_DATAGRAM so oversize packets are detectable.
    buffer: Box<[u8; 65_536]>,
    pub stats: TransportStats,
}

impl UdpTransport {
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            buffer: Box::new([0; 65_536]),
            stats: TransportStats::default(),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn send(&mut self, packet: &Packet, to: SocketAddr) -> Result<(), String> {
        let bytes = packet.encode()?;
        match self.socket.send_to(&bytes, to) {
            Ok(_) => {
                self.stats.sent_packets += 1;
                self.stats.sent_bytes += bytes.len() as u64;
                Ok(())
            }
            // Full OS buffer: drop like any lost datagram.
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => {
                self.stats.send_errors += 1;
                Err(error.to_string())
            }
        }
    }

    /// Drain up to `MAX_RECV_PER_POLL` decodable packets.
    pub fn recv_into(&mut self, out: &mut Vec<(SocketAddr, Packet)>) {
        for _ in 0..MAX_RECV_PER_POLL {
            match self.socket.recv_from(&mut self.buffer[..]) {
                Ok((size, from)) => {
                    if size > MAX_DATAGRAM {
                        self.stats.oversize_drops += 1;
                        continue;
                    }
                    self.stats.received_packets += 1;
                    self.stats.received_bytes += size as u64;
                    match Packet::decode(&self.buffer[..size]) {
                        Some(packet) => out.push((from, packet)),
                        None => self.stats.decode_drops += 1,
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                // Windows reports ICMP port-unreachable from an earlier send
                // as a recv error; it carries no packet, keep draining.
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(_) => break,
            }
        }
    }
}
