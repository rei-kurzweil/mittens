use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VrBackendPreference {
    #[default]
    Auto,
    OpenXR,
}

#[derive(Debug, Clone)]
pub struct VrComponent {
    pub enabled: bool,
    pub backend: VrBackendPreference,
}

impl Default for VrComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl VrComponent {
    pub fn new(enabled: bool, backend: VrBackendPreference) -> Self {
        Self { enabled, backend }
    }

    pub fn on() -> Self {
        Self {
            enabled: true,
            backend: VrBackendPreference::Auto,
        }
    }

    pub fn off() -> Self {
        Self {
            enabled: false,
            backend: VrBackendPreference::Auto,
        }
    }

    pub fn auto() -> Self {
        Self::on()
    }

    pub fn openxr() -> Self {
        Self {
            enabled: true,
            backend: VrBackendPreference::OpenXR,
        }
    }
}

impl Component for VrComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "vr"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterVr {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = if !self.enabled {
            "off"
        } else {
            match self.backend {
                VrBackendPreference::Auto => "on",
                VrBackendPreference::OpenXR => "openxr",
            }
        };
        ce_call("VR", ctor, vec![])
    }
}
