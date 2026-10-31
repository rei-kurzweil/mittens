use super::{Edit, MAX_PACKET, Packet};
use crate::engine::user_input::TextInputFrameEvent;
use std::{
    collections::VecDeque,
    io,
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};
struct Pending {
    packet: Packet,
    last_sent: Instant,
    attempts: usize,
}
pub struct UdpTextInputSender {
    socket: UdpSocket,
    peer: SocketAddr,
    session: uuid::Uuid,
    next: u64,
    enabled: bool,
    ready: bool,
    pending: Option<Pending>,
    queue: VecDeque<Edit>,
    heartbeat: Instant,
}
impl UdpTextInputSender {
    pub fn new(peer: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(if peer.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peer,
            session: uuid::Uuid::new_v4(),
            next: 1,
            enabled: false,
            ready: false,
            pending: None,
            queue: VecDeque::new(),
            heartbeat: Instant::now(),
        })
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.ready = false;
        self.pending = None;
        self.queue.clear();
        self.next = 1;
        self.session = uuid::Uuid::new_v4();
    }
    pub fn is_ready(&self) -> bool {
        self.enabled && self.ready
    }
    pub fn enqueue(&mut self, events: &[TextInputFrameEvent]) -> io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let mut edits = Vec::new();
        for event in events {
            if let TextInputFrameEvent::InsertText(text) = event {
                let mut chunk = String::new();
                for ch in text.chars() {
                    if chunk.len() + ch.len_utf8() > 128 {
                        edits.push(Edit::Insert(std::mem::take(&mut chunk)));
                    }
                    chunk.push(ch);
                }
                if !chunk.is_empty() {
                    edits.push(Edit::Insert(chunk));
                }
            } else {
                edits.push(Edit::from_event(event));
            }
        }
        if self.queue.len() + edits.len() > 128 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "UDP terminal input queue is full",
            ));
        }
        self.queue.extend(edits);
        Ok(())
    }
    pub fn poll(&mut self) -> io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let now = Instant::now();
        let mut buffer = [0u8; MAX_PACKET + 1];
        for _ in 0..16 {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, address)) if address == self.peer => {
                    if let Some(ack) = Packet::decode(&buffer[..size])
                        .filter(|p| p.ack && p.events.is_empty() && p.session == self.session)
                    {
                        if self
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.packet.sequence == ack.sequence)
                        {
                            if ack.sequence == 0 {
                                self.ready = true;
                            } else {
                                self.next += 1;
                            }
                            self.pending = None;
                        }
                    }
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        if let Some(pending) = self.pending.as_mut() {
            if now.duration_since(pending.last_sent) >= Duration::from_millis(50) {
                if pending.attempts >= 40 {
                    self.set_enabled(false);
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "UDP terminal did not acknowledge input",
                    ));
                }
                pending.packet.send(&self.socket, self.peer)?;
                pending.last_sent = now;
                pending.attempts += 1;
            }
            return Ok(());
        }
        let packet = if !self.ready {
            Some(Packet::new(self.session, 0, Vec::new()))
        } else if !self.queue.is_empty() {
            let mut events = Vec::new();
            while events.len() < 16 {
                let Some(event) = self.queue.front() else {
                    break;
                };
                events.push(event.clone());
                if serde_json::to_vec(&Packet::new(self.session, self.next, events.clone()))?.len()
                    > MAX_PACKET
                {
                    events.pop();
                    break;
                }
                self.queue.pop_front();
            }
            Some(Packet::new(self.session, self.next, events))
        } else if now.duration_since(self.heartbeat) >= Duration::from_millis(500) {
            Packet::new(self.session, 0, Vec::new()).send(&self.socket, self.peer)?;
            self.heartbeat = now;
            None
        } else {
            None
        };
        if let Some(packet) = packet {
            packet.send(&self.socket, self.peer)?;
            self.heartbeat = now;
            self.pending = Some(Pending {
                packet,
                last_sent: now,
                attempts: 1,
            });
        }
        Ok(())
    }
}
