use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Mirror component.
///
/// Attached to a `RenderableComponent` (or its ancestor) to enable planar reflections.
/// The mirror plane is defined by the nearest ancestor `TransformComponent`'s XY plane
/// (+Z is the reflection normal).
#[derive(Debug, Clone)]
pub struct MirrorComponent {
    /// Resolution of the mirror texture (e.g. 512, 1024).
    pub quality: i32,
    /// Freeze this mirror's captures while retaining its last displayed images.
    pub enabled: bool,
    component: Option<ComponentId>,
}

impl MirrorComponent {
    pub fn new(quality: i32) -> Self {
        Self {
            quality: quality.clamp(64, 2048),
            enabled: true,
            component: None,
        }
    }
}

impl Default for MirrorComponent {
    fn default() -> Self {
        Self::new(512)
    }
}

impl Component for MirrorComponent {
    fn name(&self) -> &'static str {
        "mirror"
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

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let authored = ce_call("Mirror", "quality", vec![num(self.quality as f64)]);
        if self.enabled {
            authored
        } else {
            authored.with_call("enabled", vec![b(false)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_pause_is_opt_in_and_preserved_by_serialization() {
        let mut mirror = MirrorComponent::default();
        assert!(mirror.enabled);
        mirror.enabled = false;
        let source = crate::scripting::unparser::unparse_component(
            &mirror.to_mms_ast(&crate::engine::ecs::World::default()),
        );
        assert!(source.contains("quality(512.0)"), "{source}");
        assert!(source.contains("enabled(false)"), "{source}");
    }
}
