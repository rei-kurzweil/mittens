//! Bounded native channel registration and latest-value telemetry.
//! No scene writes, background threads, reliability promises for samples, or
//! buffering of writes before registration. Declarations retry until acknowledged
//! and continue as heartbeats. Wire IDs never repeat within a socket epoch.
use super::protocol::{self, ACK, DECLARE, HEADER, REJECT, SAMPLE, WITHDRAW};
use super::schema::{error, validate_name};
use super::{ChannelError, MAX_DATAGRAM, Schema};
use meow_meow_script::TransportValue;
use slotmap::{SlotMap, new_key_type};
use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

const MAX_CHANNELS: usize = 64;
const MAX_PEERS: usize = 16;
const MAX_RETIRED_EPOCHS: usize = 64;
const MAX_CHANNEL_HISTORY: usize = 256;
const MAX_RECV: usize = 256;
const MAX_EVENTS: usize = 256;
const RETRY: Duration = Duration::from_millis(250);
const TIMEOUT: Duration = Duration::from_secs(3);

new_key_type! {
    /// Local publication handle; never a cross-process component identity.
    pub struct PublicationId;
    pub struct SubscriptionId;
    /// Local received-channel arena handle, shared by compatible subscriptions.
    pub struct ChannelId;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelEvent {
    PublicationReady(PublicationId),
    PublicationLost(PublicationId),
    Ready(SubscriptionId),
    Lost(SubscriptionId),
    /// Latest value is available via `sample`; updates coalesce within a poll.
    Sample(SubscriptionId),
    SchemaMismatch(SubscriptionId),
    PublicationRejected(PublicationId),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ChannelStats {
    pub received_packets: u64,
    pub sent_packets: u64,
    pub invalid_packets: u64,
    pub unknown_samples: u64,
    pub stale_samples: u64,
    pub dropped_events: u64,
    pub send_would_block: u64,
}

struct Publication {
    name: String,
    schema: Schema,
    to: SocketAddr,
    wire_id: u64,
    sequence: u64,
    declaration: Vec<u8>,
    last_declare: Option<Instant>,
    last_ack: Option<Instant>,
    rejected: bool,
}
struct Subscription {
    from: SocketAddr,
    name: String,
    schema: Schema,
    channel: Option<ChannelId>,
    mismatch: bool,
}
struct RemoteChannel {
    from: SocketAddr,
    epoch: [u8; 16],
    wire_id: u64,
    name: String,
    schema: Schema,
    sequence: u64,
    last_heard: Instant,
    payload: Box<[u8; MAX_DATAGRAM]>,
    payload_len: usize,
    updated: bool,
}
struct Peer {
    epoch: [u8; 16],
    // Never evict retired epochs: old delayed declarations must not resurrect.
    retired: Vec<[u8; 16]>,
    // Preserve identity and sequence floors across timeout/unsubscribe. Bounded
    // history fails closed rather than allowing old samples to become current.
    channels: HashMap<u64, ChannelHistory>,
}
struct ChannelHistory {
    definition: Option<(String, Schema)>,
    sequence: u64,
    withdrawn: bool,
}

/// One first-class native socket with many typed publication/subscription handles.
/// Poll and consume events/samples on the owning thread. Errors from local I/O
/// propagate; invalid network input is counted and discarded.
pub struct ChannelSocket {
    socket: UdpSocket,
    epoch: [u8; 16],
    next_wire_id: u64,
    publications: SlotMap<PublicationId, Publication>,
    subscriptions: SlotMap<SubscriptionId, Subscription>,
    channels: SlotMap<ChannelId, RemoteChannel>,
    peers: HashMap<SocketAddr, Peer>,
    events: Vec<ChannelEvent>,
    pub stats: ChannelStats,
}

pub struct Sample<'a> {
    pub channel: ChannelId,
    pub sequence: u64,
    pub schema: &'a Schema,
    pub bytes: &'a [u8],
}

impl ChannelSocket {
    pub fn bind(addr: SocketAddr) -> Result<Self, ChannelError> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            epoch: *uuid::Uuid::new_v4().as_bytes(),
            next_wire_id: 1,
            publications: SlotMap::with_key(),
            subscriptions: SlotMap::with_key(),
            channels: SlotMap::with_key(),
            peers: HashMap::new(),
            events: Vec::with_capacity(MAX_EVENTS),
            stats: ChannelStats::default(),
        })
    }
    pub fn local_addr(&self) -> Result<SocketAddr, ChannelError> {
        Ok(self.socket.local_addr()?)
    }

    pub fn publish(
        &mut self,
        to: SocketAddr,
        name: &str,
        schema: Schema,
    ) -> Result<PublicationId, ChannelError> {
        validate_name(name)?;
        if self.publications.len() == MAX_CHANNELS {
            return Err(error("publication limit reached"));
        }
        if self
            .publications
            .values()
            .any(|p| p.to == to && p.name == name)
        {
            return Err(error("duplicate publication for destination/name"));
        }
        let wire_id = self.next_wire_id;
        let next_wire_id = wire_id
            .checked_add(1)
            .ok_or_else(|| error("wire channel ID space exhausted"))?;
        let mut bytes = [0u8; MAX_DATAGRAM];
        protocol::header(&mut bytes, DECLARE, self.epoch, wire_id, 0)?;
        let len = protocol::declaration(name, &schema, &mut bytes)?;
        self.next_wire_id = next_wire_id;
        Ok(self.publications.insert(Publication {
            name: name.into(),
            schema,
            to,
            wire_id,
            sequence: 0,
            declaration: bytes[..len].to_vec(),
            last_declare: None,
            last_ack: None,
            rejected: false,
        }))
    }

    pub fn subscribe(
        &mut self,
        from: SocketAddr,
        name: &str,
        schema: Schema,
    ) -> Result<SubscriptionId, ChannelError> {
        validate_name(name)?;
        // Reject locally impossible registration before allocating state.
        protocol::declaration(name, &schema, &mut [0; MAX_DATAGRAM])?;
        if self.subscriptions.len() == MAX_CHANNELS {
            return Err(error("subscription limit reached"));
        }
        if self
            .subscriptions
            .values()
            .any(|s| s.from == from && s.name == name && s.schema != schema)
        {
            return Err(error("conflicting local subscription schemas"));
        }
        let channel = self
            .channels
            .iter()
            .find(|(_, c)| c.from == from && c.name == name && c.schema == schema)
            .map(|(id, _)| id);
        let id = self.subscriptions.insert(Subscription {
            from,
            name: name.into(),
            schema,
            channel,
            mismatch: false,
        });
        if channel.is_some() {
            self.event(ChannelEvent::Ready(id));
        }
        Ok(id)
    }

    pub fn is_publication_ready(&self, id: PublicationId) -> bool {
        self.publications
            .get(id)
            .is_some_and(|p| p.last_ack.is_some() && !p.rejected)
    }
    pub fn channel(&self, id: SubscriptionId) -> Option<ChannelId> {
        self.subscriptions.get(id)?.channel
    }
    pub fn sample(&self, id: SubscriptionId) -> Option<Sample<'_>> {
        let channel = self.channel(id)?;
        let c = self.channels.get(channel)?;
        (c.payload_len != 0).then_some(Sample {
            channel,
            sequence: c.sequence,
            schema: &c.schema,
            bytes: &c.payload[..c.payload_len],
        })
    }
    pub fn drain_events(&mut self) -> impl Iterator<Item = ChannelEvent> + '_ {
        self.events.drain(..)
    }

    /// Write one numeric sample after registration. `false` means the OS send
    /// buffer was full and this unreliable sample was dropped.
    pub fn write_numeric(
        &mut self,
        id: PublicationId,
        values: &[f32],
    ) -> Result<bool, ChannelError> {
        let p = self.ready_publication(id)?;
        let mut packet = [0; MAX_DATAGRAM];
        let len = p.schema.encode_numeric(values, &mut packet[HEADER..])?;
        self.send_sample(id, &mut packet, len)
    }
    pub fn write(
        &mut self,
        id: PublicationId,
        value: &TransportValue,
    ) -> Result<bool, ChannelError> {
        let p = self.ready_publication(id)?;
        let mut packet = [0; MAX_DATAGRAM];
        let len = p.schema.encode_value(value, &mut packet[HEADER..])?;
        self.send_sample(id, &mut packet, len)
    }
    fn ready_publication(&self, id: PublicationId) -> Result<&Publication, ChannelError> {
        let p = self
            .publications
            .get(id)
            .ok_or_else(|| error("stale publication handle"))?;
        if p.last_ack.is_none() || p.rejected {
            return Err(error("publication is not registered"));
        }
        Ok(p)
    }
    fn send_sample(
        &mut self,
        id: PublicationId,
        packet: &mut [u8],
        len: usize,
    ) -> Result<bool, ChannelError> {
        let p = self
            .publications
            .get_mut(id)
            .ok_or_else(|| error("stale publication handle"))?;
        p.sequence = p
            .sequence
            .checked_add(1)
            .ok_or_else(|| error("sequence exhausted; republish channel"))?;
        protocol::header(packet, SAMPLE, self.epoch, p.wire_id, p.sequence)?;
        send(&self.socket, &mut self.stats, &packet[..HEADER + len], p.to)
    }

    pub fn remove_publication(&mut self, id: PublicationId) -> Result<(), ChannelError> {
        let p = self
            .publications
            .remove(id)
            .ok_or_else(|| error("stale publication handle"))?;
        let mut packet = [0; HEADER];
        protocol::header(&mut packet, WITHDRAW, self.epoch, p.wire_id, 0)?;
        send(&self.socket, &mut self.stats, &packet, p.to)?;
        Ok(())
    }
    pub fn remove_subscription(&mut self, id: SubscriptionId) -> Result<(), ChannelError> {
        let s = self
            .subscriptions
            .remove(id)
            .ok_or_else(|| error("stale subscription handle"))?;
        if let Some(channel) = s.channel {
            if !self
                .subscriptions
                .values()
                .any(|s| s.channel == Some(channel))
            {
                self.channels.remove(channel);
            }
        }
        Ok(())
    }

    /// At most 256 datagrams per call. Events are bounded; samples coalesce to
    /// one latest value per channel per poll. `now` must advance monotonically.
    pub fn poll(&mut self, now: Instant) -> Result<(), ChannelError> {
        for c in self.channels.values_mut() {
            c.updated = false;
        }
        // An extra byte detects truncation of every oversize datagram on Unix.
        // Windows reports WSAEMSGSIZE instead; both cases discard the packet.
        let mut buffer = [0u8; MAX_DATAGRAM + 1];
        for _ in 0..MAX_RECV {
            let (len, from) = match self.socket.recv_from(&mut buffer) {
                Ok(value) => value,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) if e.raw_os_error() == Some(10040) => {
                    self.stats.invalid_packets += 1;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            self.stats.received_packets += 1;
            if self.receive(from, &buffer[..len], now).is_err() {
                self.stats.invalid_packets += 1;
            }
        }
        // No temporary per-poll Vec: bounded work over the channel arenas.
        for (id, c) in &self.channels {
            if c.updated {
                for (sub, s) in &self.subscriptions {
                    if s.channel == Some(id) {
                        push_event(&mut self.events, &mut self.stats, ChannelEvent::Sample(sub));
                    }
                }
            }
        }
        let expired: [_; MAX_CHANNELS] = {
            let mut ids = [None; MAX_CHANNELS];
            for (slot, (id, _)) in ids.iter_mut().zip(
                self.channels
                    .iter()
                    .filter(|(_, c)| now.saturating_duration_since(c.last_heard) >= TIMEOUT),
            ) {
                *slot = Some(id);
            }
            ids
        };
        for id in expired.into_iter().flatten() {
            self.lose_channel(id);
        }
        for (id, p) in &mut self.publications {
            if p.last_ack
                .is_some_and(|last| now.saturating_duration_since(last) >= TIMEOUT)
            {
                p.last_ack = None;
                push_event(
                    &mut self.events,
                    &mut self.stats,
                    ChannelEvent::PublicationLost(id),
                );
            }
            if p.last_declare
                .is_none_or(|last| now.saturating_duration_since(last) >= RETRY)
            {
                send(&self.socket, &mut self.stats, &p.declaration, p.to)?;
                p.last_declare = Some(now);
            }
        }
        Ok(())
    }

    fn receive(
        &mut self,
        from: SocketAddr,
        bytes: &[u8],
        now: Instant,
    ) -> Result<(), ChannelError> {
        let packet = protocol::decode(bytes)?;
        match packet.kind {
            DECLARE => self.register_remote(from, packet, now),
            ACK | REJECT => {
                if packet.epoch != self.epoch {
                    return Ok(());
                }
                if let Some((id, p)) = self
                    .publications
                    .iter_mut()
                    .find(|(_, p)| p.to == from && p.wire_id == packet.channel)
                {
                    if packet.kind == ACK {
                        let ready = p.last_ack.is_some() && !p.rejected;
                        p.last_ack = Some(now);
                        p.rejected = false;
                        if !ready {
                            self.event(ChannelEvent::PublicationReady(id));
                        }
                    } else {
                        let rejected = p.rejected;
                        p.last_ack = None;
                        p.rejected = true;
                        if !rejected {
                            self.event(ChannelEvent::PublicationRejected(id));
                        }
                    }
                }
                Ok(())
            }
            SAMPLE => {
                let Some((_, c)) = self.channels.iter_mut().find(|(_, c)| {
                    c.from == from && c.epoch == packet.epoch && c.wire_id == packet.channel
                }) else {
                    self.stats.unknown_samples += 1;
                    return Ok(());
                };
                if packet.sequence <= c.sequence {
                    self.stats.stale_samples += 1;
                    return Ok(());
                }
                c.schema.validate_payload(packet.payload)?;
                c.payload[..packet.payload.len()].copy_from_slice(packet.payload);
                c.payload_len = packet.payload.len();
                c.sequence = packet.sequence;
                if let Some(history) = self
                    .peers
                    .get_mut(&from)
                    .and_then(|p| p.channels.get_mut(&packet.channel))
                {
                    history.sequence = packet.sequence;
                }
                c.last_heard = now;
                c.updated = true;
                Ok(())
            }
            WITHDRAW => {
                if !self.subscriptions.values().any(|s| s.from == from)
                    || !self.accept_peer_epoch(from, packet.epoch)?
                {
                    return Ok(());
                }
                let peer = self.peers.get_mut(&from).unwrap();
                if !peer.channels.contains_key(&packet.channel) {
                    if peer.channels.len() == MAX_CHANNEL_HISTORY {
                        return Err(error("channel history limit reached; restart socket"));
                    }
                    peer.channels.insert(
                        packet.channel,
                        ChannelHistory {
                            definition: None,
                            sequence: 0,
                            withdrawn: true,
                        },
                    );
                } else {
                    peer.channels.get_mut(&packet.channel).unwrap().withdrawn = true;
                }
                let id = self
                    .channels
                    .iter()
                    .find(|(_, c)| {
                        c.from == from && c.epoch == packet.epoch && c.wire_id == packet.channel
                    })
                    .map(|(id, _)| id);
                if let Some(id) = id {
                    self.lose_channel(id);
                }
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn register_remote(
        &mut self,
        from: SocketAddr,
        packet: protocol::Packet<'_>,
        now: Instant,
    ) -> Result<(), ChannelError> {
        let (name, schema) = protocol::read_declaration(packet.payload)?;
        if !self
            .subscriptions
            .values()
            .any(|s| s.from == from && s.name == name)
            || !self.accept_peer_epoch(from, packet.epoch)?
        {
            return Ok(());
        }
        let mut matched = false;
        let mut compatible = false;
        for (id, s) in &mut self.subscriptions {
            if s.from == from && s.name == name {
                matched = true;
                if s.schema == schema {
                    compatible = true;
                } else if !s.mismatch {
                    s.mismatch = true;
                    push_event(
                        &mut self.events,
                        &mut self.stats,
                        ChannelEvent::SchemaMismatch(id),
                    );
                }
            }
        }
        if !matched {
            return Ok(());
        } // No allocation for unsolicited declarations.
        if !compatible {
            return self.reply(from, REJECT, packet.epoch, packet.channel);
        }
        let peer = self.peers.get_mut(&from).unwrap();
        let sequence_floor = if let Some(history) = peer.channels.get(&packet.channel) {
            if history.withdrawn {
                return Ok(());
            }
            if !matches!(&history.definition, Some((n, t)) if n == &name && t == &schema) {
                return Err(error("wire identity cannot change name/schema"));
            }
            history.sequence
        } else {
            if peer.channels.len() == MAX_CHANNEL_HISTORY {
                return Err(error("channel history limit reached; restart socket"));
            }
            peer.channels.insert(
                packet.channel,
                ChannelHistory {
                    definition: Some((name.clone(), schema.clone())),
                    sequence: 0,
                    withdrawn: false,
                },
            );
            0
        };
        let existing = self
            .channels
            .iter()
            .find(|(_, c)| c.from == from && c.epoch == packet.epoch && c.wire_id == packet.channel)
            .map(|(id, _)| id);
        let channel = if let Some(id) = existing {
            let c = &mut self.channels[id];
            if c.name != name || c.schema != schema {
                return Err(error(
                    "channel identity redeclared with different name/type",
                ));
            }
            c.last_heard = now;
            id
        } else {
            if self.channels.len() == MAX_CHANNELS {
                return Err(error("received channel limit reached"));
            }
            // A name cannot be rebound to another wire identity while still active.
            if self
                .channels
                .values()
                .any(|c| c.from == from && c.name == name)
            {
                return Err(error("active channel name redeclared"));
            }
            self.channels.insert(RemoteChannel {
                from,
                epoch: packet.epoch,
                wire_id: packet.channel,
                name: name.clone(),
                schema: schema.clone(),
                sequence: sequence_floor,
                last_heard: now,
                payload: Box::new([0; MAX_DATAGRAM]),
                payload_len: 0,
                updated: false,
            })
        };
        for (id, s) in &mut self.subscriptions {
            if s.from == from && s.name == name && s.schema == schema && s.channel != Some(channel)
            {
                s.channel = Some(channel);
                s.mismatch = false;
                push_event(&mut self.events, &mut self.stats, ChannelEvent::Ready(id));
            }
        }
        self.reply(from, ACK, packet.epoch, packet.channel)
    }
    fn accept_peer_epoch(
        &mut self,
        from: SocketAddr,
        epoch: [u8; 16],
    ) -> Result<bool, ChannelError> {
        if let Some(peer) = self.peers.get_mut(&from) {
            if peer.retired.contains(&epoch) {
                return Ok(false);
            }
            if peer.epoch != epoch {
                if peer.retired.len() == MAX_RETIRED_EPOCHS {
                    return Err(error("peer restart history full; restart socket"));
                }
                peer.retired.push(peer.epoch);
                peer.epoch = epoch;
                peer.channels.clear();
                let mut stale = [None; MAX_CHANNELS];
                for (slot, (id, _)) in stale
                    .iter_mut()
                    .zip(self.channels.iter().filter(|(_, c)| c.from == from))
                {
                    *slot = Some(id);
                }
                for id in stale.into_iter().flatten() {
                    self.lose_channel(id);
                }
            }
        } else {
            if self.peers.len() == MAX_PEERS {
                return Err(error("peer limit reached"));
            }
            self.peers.insert(
                from,
                Peer {
                    epoch,
                    retired: Vec::new(),
                    channels: HashMap::new(),
                },
            );
        }
        Ok(true)
    }

    fn reply(
        &mut self,
        to: SocketAddr,
        kind: u8,
        epoch: [u8; 16],
        channel: u64,
    ) -> Result<(), ChannelError> {
        let mut packet = [0; HEADER];
        protocol::header(&mut packet, kind, epoch, channel, 0)?;
        send(&self.socket, &mut self.stats, &packet, to)?;
        Ok(())
    }
    fn lose_channel(&mut self, id: ChannelId) {
        self.channels.remove(id);
        for (sub, s) in &mut self.subscriptions {
            if s.channel == Some(id) {
                s.channel = None;
                push_event(&mut self.events, &mut self.stats, ChannelEvent::Lost(sub));
            }
        }
    }
    fn event(&mut self, event: ChannelEvent) {
        push_event(&mut self.events, &mut self.stats, event);
    }
}

fn push_event(events: &mut Vec<ChannelEvent>, stats: &mut ChannelStats, event: ChannelEvent) {
    if events.len() < MAX_EVENTS {
        events.push(event);
    } else {
        stats.dropped_events += 1;
    }
}
fn send(
    socket: &UdpSocket,
    stats: &mut ChannelStats,
    bytes: &[u8],
    to: SocketAddr,
) -> Result<bool, ChannelError> {
    match socket.send_to(bytes, to) {
        Ok(len) if len == bytes.len() => {
            stats.sent_packets += 1;
            Ok(true)
        }
        Ok(_) => Err(error("partial UDP datagram send")),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
            stats.send_would_block += 1;
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}
