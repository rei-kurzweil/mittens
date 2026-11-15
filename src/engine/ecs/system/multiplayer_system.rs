//! Multiplayer session control: connections, roster, heartbeats and app messages.
//! Spatial telemetry and application bindings use typed UDP channels.
//!
//! Deliberately absent (stubbed at the marked TODOs): snapshot interpolation,
//! client prediction/reconciliation, reliable channels, interest management,
//! delta compression, late-join entity spawning, and NAT traversal.
#[path = "multiplayer_system/endpoint.rs"]
mod endpoint;
#[path = "multiplayer_system/protocol.rs"]
mod protocol;
#[cfg(test)]
#[path = "multiplayer_system/tests.rs"]
mod tests;
#[path = "multiplayer_system/transport.rs"]
mod transport;

use crate::engine::ecs::component::MultiplayerSessionComponent;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use endpoint::{MultiplayerEndpoint, MultiplayerEvent};
use std::time::Instant;

#[derive(Debug)]
struct SessionRuntime {
    component: ComponentId,
    endpoint: MultiplayerEndpoint,
    send_interval_sec: f32,
    send_accumulator_sec: f32,
    tick: u64,
}

#[derive(Debug, Default)]
pub struct MultiplayerSystem {
    session: Option<SessionRuntime>,
}

impl MultiplayerSystem {
    pub fn register_session(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        component: ComponentId,
    ) {
        self.register_session_on(
            world,
            emit,
            component,
            &crate::engine::udp::UdpSystem::default(),
        );
    }
    pub fn register_session_on(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        component: ComponentId,
        udp: &crate::engine::udp::UdpSystem,
    ) {
        let Some(config) = world.get_component_by_id_as::<MultiplayerSessionComponent>(component)
        else {
            return;
        };
        if !config.enabled {
            return;
        }
        if let Some(active) = &self.session {
            if active.component != component {
                emit.push_event(
                    component,
                    EventSignal::MultiplayerError {
                        phase: "configuration".into(),
                        message: "only one MultiplayerSession may be active per process".into(),
                    },
                );
                return;
            }
        }
        if config.tick_hz.is_finite() && config.tick_hz > 0.0 && config.tick_hz < 1.0 {
            emit.push_event(
                component,
                EventSignal::MultiplayerError {
                    phase: "configuration".into(),
                    message: "tick_hz must be at least 1 Hz to maintain the session heartbeat"
                        .into(),
                },
            );
            return;
        }
        let tick_hz = if config.tick_hz.is_finite() && config.tick_hz > 0.0 {
            config.tick_hz.min(120.0)
        } else {
            20.0
        };
        match MultiplayerEndpoint::bind_on(
            udp,
            config.role,
            &config.addr,
            &config.player_name,
            Instant::now(),
        ) {
            Ok(endpoint) => {
                if let Some(addr) = endpoint.local_addr() {
                    eprintln!(
                        "[multiplayer] {} '{}' bound {addr}",
                        if endpoint.is_host() { "host" } else { "client" },
                        config.player_name
                    );
                }
                self.session = Some(SessionRuntime {
                    component,
                    endpoint,
                    send_interval_sec: 1.0 / tick_hz,
                    send_accumulator_sec: 0.0,
                    tick: 0,
                });
            }
            Err(message) => emit.push_event(
                component,
                EventSignal::MultiplayerError {
                    phase: "bind".into(),
                    message,
                },
            ),
        }
    }

    /// Teardown for a removed `MultiplayerSession`.
    pub fn component_removed(&mut self, component: ComponentId) {
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.component == component)
        {
            // Dropping the endpoint sends Bye.
            self.session = None;
        }
    }

    pub fn send_message(
        &mut self,
        emit: &mut dyn SignalEmitter,
        component: ComponentId,
        name: &str,
        text: &str,
    ) {
        let result = match &mut self.session {
            Some(session) if session.component == component => {
                session.endpoint.send_app(name, text)
            }
            _ => Err("MultiplayerSession is not active".to_string()),
        };
        if let Err(message) = result {
            emit.push_event(
                component,
                EventSignal::MultiplayerError {
                    phase: "send".into(),
                    message,
                },
            );
        }
    }

    /// Receive session events and publish roster/heartbeat control at `tick_hz`.
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        self.tick_at(world, emit, dt_sec, Instant::now());
    }
    fn tick_at(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
        now: Instant,
    ) {
        let Some(session) = &mut self.session else {
            return;
        };
        session.endpoint.poll(now);
        let is_host = session.endpoint.is_host();
        let scope = session.component;
        if let Some(config) = world.get_component_by_id_as_mut::<MultiplayerSessionComponent>(scope)
        {
            config.peer_id = session.endpoint.local_peer_id().unwrap_or(0);
        }

        for event in session.endpoint.take_events() {
            if let MultiplayerEvent::PeerJoined { peer_id, .. } = &event {
                if let Some(ip) = session.endpoint.peer_ip(*peer_id) {
                    if let Some(c) =
                        world.get_component_by_id_as_mut::<MultiplayerSessionComponent>(scope)
                    {
                        c.peer_ips.insert(*peer_id, ip);
                    }
                }
            }
            if let MultiplayerEvent::PeerLeft { peer_id, .. } = &event {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<MultiplayerSessionComponent>(scope)
                {
                    c.peer_ips.remove(peer_id);
                }
            }

            emit.push_event(scope, to_signal(event, is_host));
        }

        session.send_accumulator_sec += dt_sec.max(0.0);
        if session.send_accumulator_sec < session.send_interval_sec {
            return;
        }
        // Never burst to catch up on missed control updates.
        session.send_accumulator_sec %= session.send_interval_sec;
        session.tick += 1;
        session.endpoint.publish(session.tick);
    }
}

fn to_signal(event: MultiplayerEvent, is_host: bool) -> EventSignal {
    match event {
        MultiplayerEvent::Connected { peer_id } => {
            EventSignal::MultiplayerConnected { peer_id, is_host }
        }
        MultiplayerEvent::PeerJoined { peer_id, name } => {
            EventSignal::MultiplayerPeerJoined { peer_id, name }
        }
        MultiplayerEvent::PeerLeft { peer_id, name } => {
            EventSignal::MultiplayerPeerLeft { peer_id, name }
        }
        MultiplayerEvent::App { from, name, text } => {
            EventSignal::MultiplayerMessage { from, name, text }
        }
        MultiplayerEvent::Error { phase, message } => EventSignal::MultiplayerError {
            phase: phase.into(),
            message,
        },
    }
}
