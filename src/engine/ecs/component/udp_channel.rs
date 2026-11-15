//! Authored socket/channel handles. Native I/O belongs to UdpChannelSystem.
use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};
use crate::engine::transform::TransformSpace;
use crate::engine::udp_channels::Schema;

#[derive(Debug)]
pub struct UdpSocketComponent {
    pub address: String,
    pub ready: bool,
    pub error: Option<String>,
    pub local_address: Option<String>,
}
impl UdpSocketComponent {
    pub fn new(address: String) -> Self {
        Self {
            address,
            ready: false,
            error: None,
            local_address: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpBinding {
    Transform {
        target: ComponentId,
        space: TransformSpace,
    },
    Morph {
        target: ComponentId,
        slot: String,
    },
}
#[derive(Debug)]
pub struct UdpChannelComponent {
    pub socket: ComponentId,
    pub publishing: bool,
    pub channel_name: String,
    pub peer: Option<std::net::SocketAddr>,
    pub schema: Option<Schema>,
    pub binding: Option<UdpBinding>,
    pub active: bool,
    pub ready: bool,
    pub error: Option<String>,
    pub pending: Option<meow_meow_script::TransportValue>,
}
impl UdpChannelComponent {
    pub fn new(socket: ComponentId, publishing: bool, channel_name: String) -> Self {
        Self {
            socket,
            publishing,
            channel_name,
            peer: None,
            schema: None,
            binding: None,
            active: false,
            ready: false,
            error: None,
            pending: None,
        }
    }
}

impl Component for UdpSocketComponent {
    fn name(&self) -> &'static str {
        "udp_socket"
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, emit: &mut dyn SignalEmitter, component_id: ComponentId) {
        emit.push_intent_now(component_id, IntentValue::RegisterUdp { component_id });
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        super::ce_helpers::ce_call(
            "UdpSocket",
            "bind",
            vec![super::ce_helpers::s(&self.address)],
        )
    }
}
impl Component for UdpChannelComponent {
    fn name(&self) -> &'static str {
        if self.publishing {
            "udp_publication"
        } else {
            "udp_subscription"
        }
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, emit: &mut dyn SignalEmitter, component_id: ComponentId) {
        emit.push_intent_now(component_id, IntentValue::RegisterUdp { component_id });
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        // Runtime-only handle: the host installs Serialize.off on these nodes.
        super::ce_helpers::ce(if self.publishing {
            "UdpPublication"
        } else {
            "UdpSubscription"
        })
    }
}
