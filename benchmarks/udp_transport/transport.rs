//! Experimental receive-buffer ownership only; no changes to engine transport.
use crate::udp::{UdpEndpoint, UdpSystem};
use crate::workload::{Kind, decode};
use std::{io, net::UdpSocket};

pub const BUDGET: usize = 256;
pub const SCRATCH: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Separate,
    Shared,
    Runtime,
}
impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Separate => "separate",
            Self::Shared => "shared",
            Self::Runtime => "runtime",
        }
    }
}

pub enum Receivers {
    Raw(Vec<UdpSocket>),
    Runtime {
        udp: UdpSystem,
        endpoints: Vec<UdpEndpoint>,
    },
}
impl Receivers {
    pub fn bind(mode: Mode, kinds: &[Kind]) -> io::Result<Self> {
        if mode == Mode::Runtime {
            let udp = UdpSystem::default();
            let endpoints = kinds
                .iter()
                .map(|kind| {
                    udp.bind(
                        "127.0.0.1:0",
                        match kind {
                            Kind::Eye | Kind::Htc => 1024,
                            Kind::Session | Kind::Avatar => 1200,
                        },
                        "benchmark",
                    )
                })
                .collect::<io::Result<Vec<_>>>()?;
            return Ok(Self::Runtime { udp, endpoints });
        }
        Ok(Self::Raw(
            kinds
                .iter()
                .map(|_| {
                    let socket = UdpSocket::bind("127.0.0.1:0")?;
                    socket.set_nonblocking(true)?;
                    Ok(socket)
                })
                .collect::<io::Result<Vec<_>>>()?,
        ))
    }
    pub fn len(&self) -> usize {
        match self {
            Self::Raw(sockets) => sockets.len(),
            Self::Runtime { endpoints, .. } => endpoints.len(),
        }
    }
    pub fn addresses(&self) -> io::Result<Vec<std::net::SocketAddr>> {
        match self {
            Self::Raw(sockets) => sockets.iter().map(|s| s.local_addr()).collect(),
            Self::Runtime { endpoints, .. } => endpoints.iter().map(|s| s.local_addr()).collect(),
        }
    }
}

pub struct Transport {
    mode: Mode,
    buffers: Vec<Box<[u8]>>,
}
impl Transport {
    pub fn new(mode: Mode, kinds: &[Kind]) -> Self {
        let sizes = match mode {
            Mode::Separate => kinds
                .iter()
                .map(|kind| match kind {
                    Kind::Eye | Kind::Htc => 1024,
                    Kind::Session | Kind::Avatar => SCRATCH,
                })
                .collect(),
            Mode::Shared => vec![SCRATCH],
            Mode::Runtime => vec![],
        };
        Self {
            mode,
            buffers: sizes
                .into_iter()
                .map(|size| vec![0; size].into_boxed_slice())
                .collect(),
        }
    }
    pub fn scratch_bytes(&self) -> usize {
        if self.mode == Mode::Runtime {
            SCRATCH
        } else {
            self.buffers.iter().map(|buffer| buffer.len()).sum()
        }
    }
    fn buffer(&mut self, route: usize) -> &mut [u8] {
        &mut self.buffers[if self.mode == Mode::Shared { 0 } else { route }]
    }
    pub fn replay(&mut self, route: usize, kind: Kind, bytes: &[u8]) -> Option<(usize, u64)> {
        let buffer = self.buffer(route);
        buffer[..bytes.len()].copy_from_slice(bytes);
        decode(kind, &buffer[..bytes.len()])
    }
    pub fn poll(
        &mut self,
        receivers: &Receivers,
        kinds: &[Kind],
        first: usize,
        mut receive: impl FnMut(Kind, Option<(usize, u64)>),
    ) -> io::Result<()> {
        let sockets = match receivers {
            Receivers::Runtime { udp, endpoints } => {
                for offset in 0..endpoints.len() {
                    let route = (first + offset) % endpoints.len();
                    udp.poll(&endpoints[route], BUDGET, |_from, bytes| {
                        receive(kinds[route], decode(kinds[route], bytes))
                    })?;
                }
                return Ok(());
            }
            Receivers::Raw(sockets) => sockets,
        };
        // Identical socket order, budgets, decoder and handler for both modes.
        // Sharing a transport does not mean sharing a wire protocol or socket.
        for offset in 0..sockets.len() {
            let route = (first + offset) % sockets.len();
            let buffer = self.buffer(route);
            for _ in 0..BUDGET {
                match sockets[route].recv_from(buffer) {
                    Ok((size, _)) => receive(kinds[route], decode(kinds[route], &buffer[..size])),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::ConnectionReset => continue,
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }
}
