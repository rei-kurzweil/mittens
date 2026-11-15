use super::{MAX_PACKET, Packet};
use crate::engine::udp::{UdpEndpoint, UdpSystem};
use crate::engine::user_input::{HidProvider, InputState};
use std::{
    collections::VecDeque,
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};
struct Controller {
    address: SocketAddr,
    session: uuid::Uuid,
    applied: u64,
    seen: Instant,
}
pub struct UdpTextInputProvider {
    socket: UdpEndpoint,
    udp: UdpSystem,
    controller: Option<Controller>,
    retired: VecDeque<uuid::Uuid>,
    last_poll: Instant,
    drain: bool,
}
impl UdpTextInputProvider {
    pub fn bind(address: SocketAddr) -> io::Result<Self> {
        Self::bind_on(&UdpSystem::default(), address)
    }
    pub fn bind_on(udp: &UdpSystem, address: SocketAddr) -> io::Result<Self> {
        let socket = udp.bind(address, MAX_PACKET, "terminal-input")?;
        Ok(Self {
            socket,
            udp: udp.clone(),
            controller: None,
            retired: VecDeque::new(),
            last_poll: Instant::now(),
            drain: false,
        })
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
    fn retire(&mut self) {
        if let Some(old) = self.controller.take() {
            self.retired.push_back(old.session);
            if self.retired.len() > 32 {
                self.retired.pop_front();
            }
        }
    }
    fn accept(
        &mut self,
        packet: Packet,
        address: SocketAddr,
        now: Instant,
        input: &mut InputState,
    ) {
        if packet.ack || self.retired.contains(&packet.session) {
            return;
        }
        let hello = packet.sequence == 0 && packet.events.is_empty();
        let same = self
            .controller
            .as_ref()
            .is_some_and(|c| c.address == address && c.session == packet.session);
        if !same {
            if !hello {
                return;
            }
            if self.controller.as_ref().is_some_and(|c| {
                c.address != address && now.duration_since(c.seen) < Duration::from_secs(3)
            }) {
                return;
            }
            self.retire();
            self.controller = Some(Controller {
                address,
                session: packet.session,
                applied: 0,
                seen: now,
            });
        }
        let controller = self.controller.as_mut().unwrap();
        controller.seen = now;
        if packet.sequence == controller.applied + 1 && !packet.events.is_empty() {
            input
                .text_input_events
                .extend(packet.events.into_iter().map(super::Edit::event));
            controller.applied += 1;
        }
        // Duplicate or out-of-order packets only acknowledge the committed prefix.
        let mut ack = Packet::new(controller.session, controller.applied, Vec::new());
        ack.ack = true;
        let _ = ack.send(&self.socket, address);
    }
}
impl HidProvider for UdpTextInputProvider {
    fn poll(&mut self) -> InputState {
        let now = Instant::now();
        if now.duration_since(self.last_poll) > Duration::from_millis(500) {
            self.retire();
            self.drain = true;
        }
        self.last_poll = now;
        let mut input = InputState::default();
        let udp = self.udp.clone();
        let socket = self.socket.clone();
        if let Ok(report) = udp.poll(&socket, 64, |address, bytes| {
            if !self.drain {
                if let Some(packet) = Packet::decode(bytes) {
                    self.accept(packet, address, now, &mut input);
                }
            }
        }) {
            if report.drained {
                self.drain = false;
            }
        }
        input
    }
}
