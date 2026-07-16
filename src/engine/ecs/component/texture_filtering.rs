use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::TextureFiltering;

#[derive(Debug, Clone, Copy)]
pub struct TextureFilteringComponent {
    pub filtering: TextureFiltering,
}

impl TextureFilteringComponent {
    pub fn new(filtering: TextureFiltering) -> Self {
        Self { filtering }
    }

    pub fn linear() -> Self {
        Self::new(TextureFiltering::Linear)
    }

    pub fn nearest() -> Self {
        Self::new(TextureFiltering::Nearest)
    }

    pub fn nearest_magnification() -> Self {
        Self::new(TextureFiltering::NearestMagnification)
    }
}

impl Default for TextureFilteringComponent {
    fn default() -> Self {
        Self::linear()
    }
}

impl Component for TextureFilteringComponent {
    fn name(&self) -> &'static str {
        "texture_filtering"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterTextureFiltering {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.filtering {
            TextureFiltering::Linear => "linear",
            TextureFiltering::Nearest => "nearest",
            TextureFiltering::NearestMagnification => "nearest_magnification",
        };
        ce_call("TextureFiltering", ctor, vec![])
    }
}
