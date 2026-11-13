use super::Component;
use crate::engine::ecs::component::ce_helpers::*;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Who writes the replicated pose of the parent transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetAuthority {
    /// Host simulates and publishes; clients apply. A client disables the
    /// nearest `Velocity` driving the transform so it never simulates locally.
    Host { key: String },
    /// This process's own player; published by whichever peer runs it.
    LocalPlayer,
    /// Proxy for another peer's `LocalPlayer`.
    RemotePlayer { peer_id: u32 },
}

/// Replicates the world pose of its parent `Transform` through the active
/// `NetSession`. Poses are applied as-received (no interpolation yet).
#[derive(Debug, Clone)]
pub struct NetSyncComponent {
    pub authority: NetAuthority,
    component: Option<ComponentId>,
}

impl NetSyncComponent {
    pub fn host(key: impl Into<String>) -> Self {
        Self::new(NetAuthority::Host { key: key.into() })
    }

    pub fn local_player() -> Self {
        Self::new(NetAuthority::LocalPlayer)
    }

    pub fn remote_player(peer_id: u32) -> Self {
        Self::new(NetAuthority::RemotePlayer { peer_id })
    }

    fn new(authority: NetAuthority) -> Self {
        Self {
            authority,
            component: None,
        }
    }
}

impl Component for NetSyncComponent {
    fn name(&self) -> &'static str {
        "net_sync"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterNetSync {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        match &self.authority {
            NetAuthority::Host { key } => ce_call("NetSync", "host", vec![s(key)]),
            NetAuthority::LocalPlayer => ce_call("NetSync", "local_player", vec![]),
            NetAuthority::RemotePlayer { peer_id } => {
                ce_call("NetSync", "remote_player", vec![num(*peer_id as f64)])
            }
        }
    }
}
