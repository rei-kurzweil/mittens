//! Shared raw UDP I/O. Protocol adapters own codecs and semantic state.
use slotmap::{SlotMap, new_key_type};
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

pub const MAX_DATAGRAM: usize = 65_535;
pub const MAX_POLL_DATAGRAMS: usize = 256;
new_key_type! { pub struct UdpEndpointId; }

#[derive(Debug, Default)]
struct Counters {
    polls: AtomicU64,
    deferred: AtomicU64,
    received: AtomicU64,
    received_bytes: AtomicU64,
    oversized: AtomicU64,
    receive_errors: AtomicU64,
    sent: AtomicU64,
    sent_bytes: AtomicU64,
    send_would_block: AtomicU64,
    send_errors: AtomicU64,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UdpStats {
    pub polls: u64,
    pub deferred_polls: u64,
    pub received_packets: u64,
    pub received_bytes: u64,
    pub oversized_packets: u64,
    pub receive_errors: u64,
    pub sent_packets: u64,
    pub sent_bytes: u64,
    pub send_would_block: u64,
    pub send_errors: u64,
}
impl Counters {
    fn snapshot(&self) -> UdpStats {
        let get = |v: &AtomicU64| v.load(Ordering::Relaxed);
        UdpStats {
            polls: get(&self.polls),
            deferred_polls: get(&self.deferred),
            received_packets: get(&self.received),
            received_bytes: get(&self.received_bytes),
            oversized_packets: get(&self.oversized),
            receive_errors: get(&self.receive_errors),
            sent_packets: get(&self.sent),
            sent_bytes: get(&self.sent_bytes),
            send_would_block: get(&self.send_would_block),
            send_errors: get(&self.send_errors),
        }
    }
}
#[derive(Debug, Default)]
struct Shared {
    scratch: OnceLock<Mutex<Box<[u8]>>>,
    endpoints: Mutex<SlotMap<UdpEndpointId, Weak<Endpoint>>>,
}
#[derive(Debug)]
struct Endpoint {
    socket: UdpSocket,
    owner: Arc<Shared>,
    limit: usize,
    label: &'static str,
    counters: Counters,
}
/// Cloneable scene/process transport owner. Clones share one scratch buffer.
#[derive(Debug, Default, Clone)]
pub struct UdpSystem {
    shared: Arc<Shared>,
}
/// A socket lease. Final drop closes the socket; diagnostics do not retain it.
#[derive(Debug, Clone)]
pub struct UdpEndpoint {
    id: UdpEndpointId,
    inner: Arc<Endpoint>,
}
#[derive(Debug)]
pub struct UdpEndpointInfo {
    pub id: UdpEndpointId,
    pub label: &'static str,
    pub local_addr: SocketAddr,
    pub max_datagram: usize,
    pub stats: UdpStats,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct UdpPoll {
    pub received: usize,
    pub delivered: usize,
    pub oversized: usize,
    pub exhausted: bool,
    pub deferred: bool,
    pub drained: bool,
}
impl UdpSystem {
    /// Cloned handles share an owner; distinct default instances do not.
    pub fn same_transport(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    pub fn bind(
        &self,
        address: impl ToSocketAddrs,
        max_datagram: usize,
        label: &'static str,
    ) -> io::Result<UdpEndpoint> {
        if !(1..=MAX_DATAGRAM).contains(&max_datagram) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UDP payload limit outside 1..=65535",
            ));
        }
        let socket = UdpSocket::bind(address)?;
        self.shared
            .scratch
            .get_or_init(|| Mutex::new(vec![0; MAX_DATAGRAM + 1].into_boxed_slice()));
        socket.set_nonblocking(true)?;
        let inner = Arc::new(Endpoint {
            socket,
            owner: Arc::clone(&self.shared),
            limit: max_datagram,
            label,
            counters: Counters::default(),
        });
        let mut endpoints = self
            .shared
            .endpoints
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        endpoints.retain(|_, endpoint| endpoint.strong_count() != 0);
        let id = endpoints.insert(Arc::downgrade(&inner));
        Ok(UdpEndpoint { id, inner })
    }
    pub fn shares_transport_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
    /// Bounded direct dispatch. A callback may send replies, but recursive or
    /// concurrent polling returns deferred instead of blocking on shared scratch.
    pub fn poll(
        &self,
        endpoint: &UdpEndpoint,
        budget: usize,
        mut receive: impl FnMut(SocketAddr, &[u8]),
    ) -> io::Result<UdpPoll> {
        if !Arc::ptr_eq(&self.shared, &endpoint.inner.owner) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UDP endpoint belongs to another transport",
            ));
        }
        let counters = &endpoint.inner.counters;
        counters.polls.fetch_add(1, Ordering::Relaxed);
        let scratch = self
            .shared
            .scratch
            .get()
            .expect("bound endpoints initialize receive storage");
        let mut scratch = match scratch.try_lock() {
            Ok(scratch) => scratch,
            Err(std::sync::TryLockError::WouldBlock) => {
                counters.deferred.fetch_add(1, Ordering::Relaxed);
                return Ok(UdpPoll {
                    deferred: true,
                    ..UdpPoll::default()
                });
            }
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
        };
        let budget = budget.min(MAX_POLL_DATAGRAMS);
        let mut report = UdpPoll::default();
        let mut bytes = 0u64;
        let mut failure = None;
        for _ in 0..budget {
            match endpoint.inner.socket.recv_from(&mut scratch) {
                Ok((len, from)) => {
                    report.received += 1;
                    bytes += len as u64;
                    if len > endpoint.inner.limit {
                        report.oversized += 1;
                        continue;
                    }
                    report.delivered += 1;
                    receive(from, &scratch[..len]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    report.drained = true;
                    break;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(error) if error.raw_os_error() == Some(10040) => {
                    report.received += 1;
                    report.oversized += 1;
                }
                Err(error) => {
                    counters.receive_errors.fetch_add(1, Ordering::Relaxed);
                    failure = Some(error);
                    break;
                }
            }
        }
        report.exhausted = !report.drained && failure.is_none() && !report.deferred;
        counters
            .received
            .fetch_add(report.received as u64, Ordering::Relaxed);
        counters.received_bytes.fetch_add(bytes, Ordering::Relaxed);
        counters
            .oversized
            .fetch_add(report.oversized as u64, Ordering::Relaxed);
        match failure {
            Some(error) => Err(error),
            None => Ok(report),
        }
    }
    /// Allocates only when explicitly inspected, never from the poll hot path.
    pub fn diagnostics(&self) -> Vec<UdpEndpointInfo> {
        let mut endpoints = self
            .shared
            .endpoints
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        endpoints.retain(|_, endpoint| endpoint.strong_count() != 0);
        endpoints
            .iter()
            .filter_map(|(id, weak)| {
                let endpoint = weak.upgrade()?;
                Some(UdpEndpointInfo {
                    id,
                    label: endpoint.label,
                    local_addr: endpoint.socket.local_addr().ok()?,
                    max_datagram: endpoint.limit,
                    stats: endpoint.counters.snapshot(),
                })
            })
            .collect()
    }
}
impl UdpEndpoint {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.socket.local_addr()
    }
    pub fn stats(&self) -> UdpStats {
        self.inner.counters.snapshot()
    }
    pub fn id(&self) -> UdpEndpointId {
        self.id
    }
    pub fn send_to(&self, bytes: &[u8], peer: SocketAddr) -> io::Result<usize> {
        let counters = &self.inner.counters;
        if bytes.len() > self.inner.limit {
            counters.send_errors.fetch_add(1, Ordering::Relaxed);
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UDP datagram exceeds endpoint payload limit",
            ));
        }
        match self.inner.socket.send_to(bytes, peer) {
            Ok(len) if len == bytes.len() => {
                counters.sent.fetch_add(1, Ordering::Relaxed);
                counters.sent_bytes.fetch_add(len as u64, Ordering::Relaxed);
                Ok(len)
            }
            Ok(_) => {
                counters.send_errors.fetch_add(1, Ordering::Relaxed);
                Err(io::Error::other("partial UDP datagram send"))
            }
            Err(error) => {
                if error.kind() == io::ErrorKind::WouldBlock {
                    counters.send_would_block.fetch_add(1, Ordering::Relaxed);
                } else {
                    counters.send_errors.fetch_add(1, Ordering::Relaxed);
                }
                Err(error)
            }
        }
    }
}
#[cfg(test)]
mod tests;
