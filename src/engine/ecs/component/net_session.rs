use super::Component;
use crate::engine::ecs::component::ce_helpers::*;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Default UDP port for `NetSession.host()` / `NetSession.join()` without a port.
pub const NET_DEFAULT_PORT: u16 = 7010;

/// Which side of a session this process plays. The host owns world/physics
/// authority; clients own only their local player pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetRole {
    #[default]
    Host,
    Client,
}

/// One multiplayer session endpoint. `addr` is the bind address for a host and
/// the host's address for a client.
#[derive(Debug, Clone)]
pub struct NetSessionComponent {
    pub role: NetRole,
    pub addr: String,
    pub player_name: String,
    /// Snapshot/pose send rate. Receiving is polled every frame.
    pub tick_hz: f32,
    pub enabled: bool,
    /// Runtime: this process's peer id once connected (host is 1), else 0.
    pub peer_id: u32,
    component: Option<ComponentId>,
}

impl Default for NetSessionComponent {
    fn default() -> Self {
        Self {
            role: NetRole::Host,
            addr: format!("0.0.0.0:{NET_DEFAULT_PORT}"),
            player_name: "player".to_string(),
            tick_hz: 20.0,
            enabled: true,
            peer_id: 0,
            component: None,
        }
    }
}

impl NetSessionComponent {
    pub fn host(bind_addr: impl Into<String>) -> Self {
        Self {
            role: NetRole::Host,
            addr: bind_addr.into(),
            ..Self::default()
        }
    }

    pub fn join(host_addr: impl Into<String>) -> Self {
        Self {
            role: NetRole::Client,
            addr: host_addr.into(),
            ..Self::default()
        }
    }

    /// Development launcher configuration:
    /// `MITTENS_NET_ROLE` (`host` | `client`, default `host`),
    /// `MITTENS_NET_ADDR` (bind address for host, host address for client),
    /// `MITTENS_NET_NAME` (player name).
    pub fn from_env() -> Result<Self, String> {
        let role = match std::env::var("MITTENS_NET_ROLE").as_deref() {
            Ok("host") | Err(_) => NetRole::Host,
            Ok("client" | "join") => NetRole::Client,
            Ok(other) => {
                return Err(format!(
                    "MITTENS_NET_ROLE must be 'host' or 'client', got '{other}'"
                ));
            }
        };
        let addr = std::env::var("MITTENS_NET_ADDR").unwrap_or_else(|_| match role {
            NetRole::Host => format!("0.0.0.0:{NET_DEFAULT_PORT}"),
            NetRole::Client => format!("127.0.0.1:{NET_DEFAULT_PORT}"),
        });
        let mut session = Self {
            role,
            addr,
            ..Self::default()
        };
        if let Ok(name) = std::env::var("MITTENS_NET_NAME") {
            session.player_name = name;
        }
        Ok(session)
    }

    pub fn with_player_name(mut self, name: impl Into<String>) -> Self {
        self.player_name = name.into();
        self
    }

    pub fn with_tick_hz(mut self, tick_hz: f32) -> Self {
        self.tick_hz = tick_hz;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn is_host(&self) -> bool {
        self.role == NetRole::Host
    }
}

impl Component for NetSessionComponent {
    fn name(&self) -> &'static str {
        "net_session"
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
            IntentValue::RegisterNetSession {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        let ctor = match self.role {
            NetRole::Host => "host",
            NetRole::Client => "join",
        };
        let mut ce = ce_call("NetSession", ctor, vec![s(&self.addr)])
            .with_call("player_name", vec![s(&self.player_name)]);
        if self.tick_hz != Self::default().tick_hz {
            ce = ce.with_call("tick_hz", vec![num(self.tick_hz as f64)]);
        }
        if !self.enabled {
            ce = ce.with_call("enabled", vec![b(false)]);
        }
        ce
    }
}
