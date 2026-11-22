use super::{Component, ComponentRef};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Declares a single-seat attachment destination for a Rider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountableComponent {
    pub entry_zone: Option<ComponentRef>,
    pub mount_anchor: Option<ComponentRef>,
    pub dismount_anchor: Option<ComponentRef>,
    /// A free seat takes any registered Rider whose movement root enters this
    /// zone, without a grip (jumping into a truck bed).
    pub auto_mount_zone: Option<ComponentRef>,
    pub on_grip: bool,
    pub enabled: bool,
}

impl Default for MountableComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl MountableComponent {
    pub fn new() -> Self {
        Self {
            entry_zone: None,
            mount_anchor: None,
            dismount_anchor: None,
            auto_mount_zone: None,
            on_grip: true,
            enabled: true,
        }
    }

    pub fn entry_zone(mut self, value: ComponentRef) -> Self {
        self.entry_zone = Some(value);
        self
    }

    pub fn mount_anchor(mut self, value: ComponentRef) -> Self {
        self.mount_anchor = Some(value);
        self
    }

    pub fn dismount_anchor(mut self, value: ComponentRef) -> Self {
        self.dismount_anchor = Some(value);
        self
    }

    pub fn auto_mount_zone(mut self, value: ComponentRef) -> Self {
        self.auto_mount_zone = Some(value);
        self
    }

    pub fn on_grip(mut self) -> Self {
        self.on_grip = true;
        self
    }

    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}

impl Component for MountableComponent {
    fn name(&self) -> &'static str {
        "mountable"
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
            IntentValue::RegisterMountable {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        use crate::scripting::ast::Expression;

        fn reference(value: &ComponentRef) -> Expression {
            match value {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            }
        }

        let mut expression = ce("Mountable");
        if let Some(value) = &self.entry_zone {
            expression = expression.with_call("entry_zone", vec![reference(value)]);
        }
        if let Some(value) = &self.mount_anchor {
            expression = expression.with_call("mount_anchor", vec![reference(value)]);
        }
        if let Some(value) = &self.dismount_anchor {
            expression = expression.with_call("dismount_anchor", vec![reference(value)]);
        }
        if let Some(value) = &self.auto_mount_zone {
            expression = expression.with_call("auto_mount_zone", vec![reference(value)]);
        }
        if self.on_grip {
            expression = expression.with_call("on_grip", vec![]);
        }
        if !self.enabled {
            expression = expression.with_call("enabled", vec![b(false)]);
        }
        expression
    }
}
