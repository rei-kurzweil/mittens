//! Session state for one host or client, independent of the ECS so it can be
//! exercised over loopback in tests.
//!
//! The host owns the roster. Session control and app messages are independent
//! of spatial telemetry, which uses typed UDP channels.
use super::protocol::{HOST_PEER_ID, Message, Packet, PeerInfo, roster_fits, snapshot_chunks};
use super::transport::UdpTransport;
use crate::engine::ecs::component::MultiplayerRole;
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub const PEER_TIMEOUT: Duration = Duration::from_secs(3);
const HELLO_INTERVAL: Duration = Duration::from_millis(500);
const MAX_PEERS: usize = 16;
const MAX_NAME_BYTES: usize = 64;
const MAX_APP_TEXT_BYTES: usize = 900;

#[derive(Debug, Clone, PartialEq)]
pub enum MultiplayerEvent {
    Connected {
        peer_id: u32,
    },
    PeerJoined {
        peer_id: u32,
        name: String,
    },
    PeerLeft {
        peer_id: u32,
        name: String,
    },
    App {
        from: u32,
        name: String,
        text: String,
    },
    Error {
        phase: &'static str,
        message: String,
    },
}

#[derive(Debug)]
struct ClientPeer {
    id: u32,
    name: String,
    session: uuid::Uuid,
    last_heard: Instant,
}

#[derive(Debug)]
enum RoleState {
    Host {
        clients: HashMap<SocketAddr, ClientPeer>,
        next_peer_id: u32,
    },
    Client {
        host_addr: SocketAddr,
        /// Host session we are welcomed into; `None` while handshaking.
        host_session: Option<uuid::Uuid>,
        last_hello: Option<Instant>,
        last_heard: Instant,
        last_tick: u64,
    },
}

#[derive(Debug)]
pub struct MultiplayerEndpoint {
    transport: UdpTransport,
    session: uuid::Uuid,
    seq: u64,
    local_name: String,
    local_peer_id: Option<u32>,
    state: RoleState,
    /// Remote peers as this process knows them (never includes itself).
    roster: BTreeMap<u32, String>,
    events: Vec<MultiplayerEvent>,
    recv_scratch: Vec<(SocketAddr, Packet)>,
}

impl MultiplayerEndpoint {
    pub fn peer_ip(&self, peer_id: u32) -> Option<String> {
        match &self.state {
            RoleState::Host { clients, .. } => clients
                .iter()
                .find(|(_, p)| p.id == peer_id)
                .map(|(addr, _)| addr.ip().to_string()),
            RoleState::Client { host_addr, .. } if peer_id == HOST_PEER_ID => {
                Some(host_addr.ip().to_string())
            }
            _ => None,
        }
    }

    pub fn bind(
        role: MultiplayerRole,
        addr: &str,
        name: &str,
        now: Instant,
    ) -> Result<Self, String> {
        Self::bind_on(
            &crate::engine::udp::UdpSystem::default(),
            role,
            addr,
            name,
            now,
        )
    }
    pub fn bind_on(
        udp: &crate::engine::udp::UdpSystem,
        role: MultiplayerRole,
        addr: &str,
        name: &str,
        now: Instant,
    ) -> Result<Self, String> {
        let addr: SocketAddr = addr
            .parse()
            .map_err(|e| format!("'{addr}' is not a numeric socket address: {e}"))?;
        let (bind_addr, state) = match role {
            MultiplayerRole::Host => (
                addr,
                RoleState::Host {
                    clients: HashMap::new(),
                    next_peer_id: HOST_PEER_ID + 1,
                },
            ),
            MultiplayerRole::Client => {
                let any: SocketAddr = if addr.is_ipv4() {
                    "0.0.0.0:0".parse().unwrap()
                } else {
                    "[::]:0".parse().unwrap()
                };
                (
                    any,
                    RoleState::Client {
                        host_addr: addr,
                        host_session: None,
                        last_hello: None,
                        last_heard: now,
                        last_tick: 0,
                    },
                )
            }
        };
        let transport = UdpTransport::bind_on(udp, bind_addr).map_err(|e| e.to_string())?;
        let mut endpoint = Self {
            transport,
            session: uuid::Uuid::new_v4(),
            seq: 0,
            local_name: truncate(name, MAX_NAME_BYTES),
            local_peer_id: None,
            state,
            roster: BTreeMap::new(),
            events: Vec::new(),
            recv_scratch: Vec::new(),
        };
        if role == MultiplayerRole::Host {
            endpoint.local_peer_id = Some(HOST_PEER_ID);
            endpoint.events.push(MultiplayerEvent::Connected {
                peer_id: HOST_PEER_ID,
            });
        }
        Ok(endpoint)
    }

    pub fn is_host(&self) -> bool {
        matches!(self.state, RoleState::Host { .. })
    }

    pub fn local_peer_id(&self) -> Option<u32> {
        self.local_peer_id
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.transport.local_addr().ok()
    }

    pub fn take_events(&mut self) -> Vec<MultiplayerEvent> {
        std::mem::take(&mut self.events)
    }

    /// Receive, handshake, and expire peers. Call once per frame.
    pub fn poll(&mut self, now: Instant) {
        let mut packets = std::mem::take(&mut self.recv_scratch);
        self.transport.recv_into(&mut packets);
        for (from, packet) in packets.drain(..) {
            if self.is_host() {
                self.host_receive(from, packet, now);
            } else {
                self.client_receive(from, packet, now);
            }
        }
        self.recv_scratch = packets;
        self.expire_and_handshake(now);
    }

    /// Publish roster control on the host, or a heartbeat on a welcomed client.
    pub fn publish(&mut self, tick: u64) {
        match &self.state {
            RoleState::Host { clients, .. } => {
                if clients.is_empty() {
                    return;
                }
                let mut peers = vec![PeerInfo {
                    id: HOST_PEER_ID,
                    name: self.local_name.clone(),
                }];
                for client in clients.values() {
                    peers.push(PeerInfo {
                        id: client.id,
                        name: client.name.clone(),
                    });
                }
                let targets: Vec<_> = clients.keys().copied().collect();
                for message in snapshot_chunks(tick, peers, Vec::new(), Vec::new()) {
                    for &to in &targets {
                        self.send(message.clone(), to, "snapshot");
                    }
                }
            }
            RoleState::Client {
                host_addr,
                host_session: Some(_),
                ..
            } => {
                let host_addr = *host_addr;
                self.send(Message::Heartbeat, host_addr, "heartbeat");
            }
            RoleState::Client { .. } => {}
        }
    }

    /// Queue an unreliable app message: client -> host, host -> all clients.
    pub fn send_app(&mut self, name: &str, text: &str) -> Result<(), String> {
        if text.len() > MAX_APP_TEXT_BYTES || name.len() > MAX_NAME_BYTES {
            return Err(format!(
                "multiplayer message too large (name <= {MAX_NAME_BYTES}, text <= {MAX_APP_TEXT_BYTES} bytes)"
            ));
        }
        let Some(from) = self.local_peer_id else {
            return Err("multiplayer session is not connected".into());
        };
        let message = Message::App {
            from,
            name: name.to_string(),
            text: text.to_string(),
        };
        match &self.state {
            RoleState::Host { clients, .. } => {
                let targets: Vec<_> = clients.keys().copied().collect();
                for to in targets {
                    self.send(message.clone(), to, "message");
                }
            }
            RoleState::Client { host_addr, .. } => {
                let host_addr = *host_addr;
                self.send(message, host_addr, "message");
            }
        }
        Ok(())
    }

    fn send(&mut self, msg: Message, to: SocketAddr, phase: &'static str) {
        self.seq += 1;
        let packet = Packet {
            v: super::protocol::VERSION,
            session: self.session,
            seq: self.seq,
            msg,
        };
        if let Err(message) = self.transport.send(&packet, to) {
            // TODO(multiplayer): coalesce repeated errors instead of one per packet.
            self.events.push(MultiplayerEvent::Error { phase, message });
        }
    }

    fn host_receive(&mut self, from: SocketAddr, packet: Packet, now: Instant) {
        let RoleState::Host {
            clients,
            next_peer_id,
        } = &mut self.state
        else {
            return;
        };
        if let Message::Hello { name } = packet.msg {
            if clients
                .get(&from)
                .is_some_and(|existing| existing.session != packet.session)
            {
                // Same address, restarted client: retire the old peer.
                let old = clients.remove(&from).unwrap();
                self.roster.remove(&old.id);
                self.events.push(MultiplayerEvent::PeerLeft {
                    peer_id: old.id,
                    name: old.name,
                });
            }
            let peer_id = if let Some(existing) = clients.get_mut(&from) {
                // Welcome was lost; repeat it.
                existing.last_heard = now;
                existing.id
            } else {
                if clients.len() >= MAX_PEERS {
                    return;
                }
                let id = *next_peer_id;
                let name = truncate(&name, MAX_NAME_BYTES);
                let mut peers = vec![PeerInfo {
                    id: HOST_PEER_ID,
                    name: self.local_name.clone(),
                }];
                peers.extend(clients.values().map(|peer| PeerInfo {
                    id: peer.id,
                    name: peer.name.clone(),
                }));
                peers.push(PeerInfo {
                    id,
                    name: name.clone(),
                });
                if !roster_fits(peers) {
                    self.events.push(MultiplayerEvent::Error {
                        phase: "admission",
                        message:
                            "player name would exceed the complete session roster datagram budget"
                                .into(),
                    });
                    return;
                }
                *next_peer_id += 1;
                clients.insert(
                    from,
                    ClientPeer {
                        id,
                        name: name.clone(),
                        session: packet.session,
                        last_heard: now,
                    },
                );
                self.roster.insert(id, name.clone());
                self.events
                    .push(MultiplayerEvent::PeerJoined { peer_id: id, name });
                id
            };
            self.send(Message::Welcome { peer_id }, from, "welcome");
            return;
        }
        // Everything else requires a welcomed sender from this session.
        let Some(client) = clients
            .get_mut(&from)
            .filter(|client| client.session == packet.session)
        else {
            return;
        };
        client.last_heard = now;
        match packet.msg {
            Message::App { name, text, .. } => {
                // Never trust the claimed sender id.
                let from = client.id;
                self.events.push(MultiplayerEvent::App { from, name, text });
            }
            Message::Bye => {
                let client = clients.remove(&from).unwrap();
                self.roster.remove(&client.id);
                self.events.push(MultiplayerEvent::PeerLeft {
                    peer_id: client.id,
                    name: client.name,
                });
            }
            _ => {}
        }
    }

    fn client_receive(&mut self, from: SocketAddr, packet: Packet, now: Instant) {
        let RoleState::Client {
            host_addr,
            host_session,
            last_heard,
            last_tick,
            ..
        } = &mut self.state
        else {
            return;
        };
        if from != *host_addr {
            return;
        }
        match packet.msg {
            Message::Welcome { peer_id } => {
                if *host_session == Some(packet.session) {
                    *last_heard = now;
                    return;
                }
                if host_session.is_some() {
                    // Host restarted under a new session.
                    self.reset_client_view();
                }
                let RoleState::Client {
                    host_session,
                    last_heard,
                    last_tick,
                    ..
                } = &mut self.state
                else {
                    return;
                };
                *host_session = Some(packet.session);
                *last_heard = now;
                *last_tick = 0;
                self.local_peer_id = Some(peer_id);
                self.events.push(MultiplayerEvent::Connected { peer_id });
            }
            msg if *host_session == Some(packet.session) => {
                *last_heard = now;
                match msg {
                    Message::Snapshot { tick, peers, .. } => {
                        // Chunks of one tick share it; anything older is stale.
                        if tick < *last_tick {
                            return;
                        }
                        *last_tick = tick;
                        if let Some(peers) = peers {
                            self.apply_roster(peers);
                        }
                    }
                    Message::App { from, name, text } => {
                        self.events.push(MultiplayerEvent::App { from, name, text });
                    }
                    Message::Bye => {
                        self.reset_client_view();
                        self.events.push(MultiplayerEvent::Error {
                            phase: "disconnect",
                            message: "host closed the session".into(),
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn apply_roster(&mut self, peers: Vec<PeerInfo>) {
        let me = self.local_peer_id;
        let next: BTreeMap<u32, String> = peers
            .into_iter()
            .filter(|peer| Some(peer.id) != me)
            .map(|peer| (peer.id, peer.name))
            .collect();
        for (&id, name) in &self.roster {
            if !next.contains_key(&id) {
                self.events.push(MultiplayerEvent::PeerLeft {
                    peer_id: id,
                    name: name.clone(),
                });
            }
        }
        for (&id, name) in &next {
            if !self.roster.contains_key(&id) {
                self.events.push(MultiplayerEvent::PeerJoined {
                    peer_id: id,
                    name: name.clone(),
                });
            }
        }
        self.roster = next;
    }

    /// Forget the host and every remote peer; the next poll re-handshakes.
    fn reset_client_view(&mut self) {
        for (id, name) in std::mem::take(&mut self.roster) {
            self.events
                .push(MultiplayerEvent::PeerLeft { peer_id: id, name });
        }
        self.local_peer_id = None;
        if let RoleState::Client {
            host_session,
            last_hello,
            ..
        } = &mut self.state
        {
            *host_session = None;
            *last_hello = None;
        }
    }

    fn expire_and_handshake(&mut self, now: Instant) {
        match &mut self.state {
            RoleState::Host { clients, .. } => {
                let expired: Vec<_> = clients
                    .iter()
                    .filter(|(_, c)| now.duration_since(c.last_heard) > PEER_TIMEOUT)
                    .map(|(addr, _)| *addr)
                    .collect();
                for addr in expired {
                    let client = clients.remove(&addr).unwrap();
                    self.roster.remove(&client.id);
                    self.events.push(MultiplayerEvent::PeerLeft {
                        peer_id: client.id,
                        name: client.name,
                    });
                }
            }
            RoleState::Client {
                host_addr,
                host_session,
                last_hello,
                last_heard,
                ..
            } => {
                if host_session.is_some() {
                    if now.duration_since(*last_heard) > PEER_TIMEOUT {
                        self.reset_client_view();
                        self.events.push(MultiplayerEvent::Error {
                            phase: "timeout",
                            message: "no packets from host; reconnecting".into(),
                        });
                    }
                    return;
                }
                if last_hello.is_none_or(|at| now.duration_since(at) >= HELLO_INTERVAL) {
                    *last_hello = Some(now);
                    *last_heard = now;
                    let host_addr = *host_addr;
                    let name = self.local_name.clone();
                    self.send(Message::Hello { name }, host_addr, "hello");
                }
            }
        }
    }
}

impl Drop for MultiplayerEndpoint {
    fn drop(&mut self) {
        let targets: Vec<SocketAddr> = match &self.state {
            RoleState::Host { clients, .. } => clients.keys().copied().collect(),
            RoleState::Client {
                host_addr,
                host_session: Some(_),
                ..
            } => vec![*host_addr],
            RoleState::Client { .. } => Vec::new(),
        };
        for to in targets {
            self.send(Message::Bye, to, "bye");
        }
    }
}

fn truncate(text: &str, max_bytes: usize) -> String {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}
