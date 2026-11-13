//! Multiplayer first slice: one `MultiplayerSession` per process, host-authoritative
//! world entities, peer-owned player poses, and app messages over UDP.
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

use crate::engine::ecs::component::{
    MultiplayerAuthority, MultiplayerSessionComponent, MultiplayerSyncComponent,
    TransformComponent, VelocityComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::transform::{TransformSpace, TransformTrs};
use endpoint::{MultiplayerEndpoint, MultiplayerEvent};
use protocol::Pose;
use std::collections::BTreeMap;
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
    /// Registered `MultiplayerSync` components. Ordered so publishing is deterministic.
    syncs: BTreeMap<ComponentId, MultiplayerAuthority>,
}

impl MultiplayerSystem {
    pub fn register_session(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        component: ComponentId,
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
        let tick_hz = if config.tick_hz.is_finite() && config.tick_hz > 0.0 {
            config.tick_hz.min(120.0)
        } else {
            20.0
        };
        match MultiplayerEndpoint::bind(
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

    pub fn register_sync(&mut self, world: &World, component: ComponentId) {
        if let Some(sync) = world.get_component_by_id_as::<MultiplayerSyncComponent>(component) {
            self.syncs.insert(component, sync.authority.clone());
        }
    }

    /// Teardown for a removed `MultiplayerSession` or `MultiplayerSync`.
    pub fn component_removed(&mut self, component: ComponentId) {
        self.syncs.remove(&component);
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

    /// Receive and apply remote state, then publish owned state at `tick_hz`.
    /// Runs after fixed-step physics so the host publishes this frame's result.
    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        let Some(session) = &mut self.session else {
            return;
        };
        session.endpoint.poll(Instant::now());
        let is_host = session.endpoint.is_host();
        let scope = session.component;

        for event in session.endpoint.take_events() {
            if let MultiplayerEvent::Connected { peer_id } = &event {
                if let Some(config) =
                    world.get_component_by_id_as_mut::<MultiplayerSessionComponent>(scope)
                {
                    config.peer_id = *peer_id;
                }
            }
            emit.push_event(scope, to_signal(event, is_host));
        }

        // Apply latest received poses as-is.
        // TODO(multiplayer): buffer snapshots and interpolate ~100 ms behind; extrapolate on loss.
        let players = session.endpoint.take_remote_players();
        let entities = session.endpoint.take_host_entities();
        for (&sync, authority) in &self.syncs {
            let pose = match authority {
                MultiplayerAuthority::RemotePlayer { peer_id } => players.get(peer_id),
                MultiplayerAuthority::Host { key } if !is_host => {
                    disable_local_simulation(world, sync);
                    entities.get(key)
                }
                _ => None,
            };
            if let (Some(pose), Some(target)) = (pose, sync_target(world, sync)) {
                emit.push_intent_now(
                    target,
                    IntentValue::SetTransformTrs {
                        component_id: target,
                        space: TransformSpace::World,
                        value: TransformTrs {
                            translation: pose.p,
                            rotation_quat_xyzw: pose.r,
                            scale: pose.s,
                        },
                    },
                );
            }
        }

        session.send_accumulator_sec += dt_sec.max(0.0);
        if session.send_accumulator_sec < session.send_interval_sec {
            return;
        }
        // Never burst to catch up; a newer pose supersedes missed sends.
        session.send_accumulator_sec %= session.send_interval_sec;
        session.tick += 1;
        let mut local_player = None;
        let mut owned_entities = Vec::new();
        for (&sync, authority) in &self.syncs {
            let pose = || sync_target(world, sync).and_then(|t| world_pose(world, t));
            match authority {
                MultiplayerAuthority::LocalPlayer if local_player.is_none() => {
                    local_player = pose()
                }
                MultiplayerAuthority::Host { key } if is_host => {
                    if let Some(pose) = pose() {
                        owned_entities.push((key.clone(), pose));
                    }
                }
                _ => {}
            }
        }
        session
            .endpoint
            .publish(session.tick, local_player, owned_entities);
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

/// A `MultiplayerSync` replicates its parent transform.
fn sync_target(world: &World, sync: ComponentId) -> Option<ComponentId> {
    world.parent_of(sync).filter(|&parent| {
        world
            .get_component_by_id_as::<TransformComponent>(parent)
            .is_some()
    })
}

fn world_pose(world: &World, target: ComponentId) -> Option<Pose> {
    let trs = TransformSystem::world_trs(world, target).ok()?;
    let pose = Pose {
        p: trs.translation,
        r: trs.rotation_quat_xyzw,
        s: trs.scale,
    };
    pose.is_finite().then_some(pose)
}

/// Clients never integrate host-owned bodies: `Velocity { T { MultiplayerSync.host(..) } }`.
fn disable_local_simulation(world: &mut World, sync: ComponentId) {
    let Some(velocity) = sync_target(world, sync).and_then(|t| world.parent_of(t)) else {
        return;
    };
    if let Some(velocity) = world.get_component_by_id_as_mut::<VelocityComponent>(velocity) {
        if velocity.enabled {
            velocity.enabled = false;
            velocity.zero_linear();
        }
    }
}
