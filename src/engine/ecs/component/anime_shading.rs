use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// Built-in selections implemented by the first live-controls slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingModel {
    Anime,
    Toon,
}

/// Isolated views of the existing Anime lighting calculation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AnimeLightingDebug {
    #[default]
    Anime,
    Distance,
    Diffuse,
}

impl AnimeLightingDebug {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "anime" => Ok(Self::Anime),
            "distance" => Ok(Self::Distance),
            "diffuse" => Ok(Self::Diffuse),
            _ => Err("lighting_debug: expected 'anime', 'distance', or 'diffuse'".into()),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Anime => "anime",
            Self::Distance => "distance",
            Self::Diffuse => "diffuse",
        }
    }

    pub fn gpu_value(self) -> f32 {
        match self {
            Self::Anime => 0.0,
            Self::Distance => 1.0,
            Self::Diffuse => 2.0,
        }
    }
}

/// Temporary Rust compatibility name; both authoring paths use one component.
/// Remove with the remaining legacy scene/component migration.
pub type AnimeShadingComponent = ShadingComponent;

/// Albedo-derived anime shading parameters for a descendant renderable.
///
/// Direct light selects between a tinted shade and the authored albedo. Rim
/// lighting is view-dependent and is clamped so it cannot brighten a channel
/// beyond the authored albedo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadingComponent {
    pub model: ShadingModel,
    pub lighting_debug: AnimeLightingDebug,
    pub shade_color: [f32; 3],
    pub shade_strength: f32,
    pub shade_threshold: f32,
    pub lit_threshold: f32,
    pub rim_color: [f32; 3],
    pub rim_strength: f32,
    pub rim_power: f32,
    /// Authored GLTF-scoped modifier that produced this runtime projection.
    ///
    /// `None` identifies an authored component. Projected copies keep this link
    /// so re-registering the source can update every generated renderable.
    source_component: Option<ComponentId>,
}

impl ShadingComponent {
    pub const DEFAULT_SHADE_COLOR: [f32; 3] = [0.72, 0.50, 0.54];
    pub const DEFAULT_SHADE_STRENGTH: f32 = 0.30;
    pub const DEFAULT_SHADE_THRESHOLD: f32 = 0.35;
    pub const DEFAULT_LIT_THRESHOLD: f32 = 0.55;
    pub const DEFAULT_RIM_COLOR: [f32; 3] = [1.0, 0.85, 0.92];
    pub const DEFAULT_RIM_STRENGTH: f32 = 0.18;
    pub const DEFAULT_RIM_POWER: f32 = 4.0;

    pub fn new() -> Self {
        Self {
            model: ShadingModel::Anime,
            lighting_debug: AnimeLightingDebug::Anime,
            shade_color: Self::DEFAULT_SHADE_COLOR,
            shade_strength: Self::DEFAULT_SHADE_STRENGTH,
            shade_threshold: Self::DEFAULT_SHADE_THRESHOLD,
            lit_threshold: Self::DEFAULT_LIT_THRESHOLD,
            rim_color: Self::DEFAULT_RIM_COLOR,
            rim_strength: Self::DEFAULT_RIM_STRENGTH,
            rim_power: Self::DEFAULT_RIM_POWER,
            source_component: None,
        }
    }

    pub fn toon() -> Self {
        Self {
            model: ShadingModel::Toon,
            ..Self::new()
        }
    }

    pub(crate) fn projected_from(mut self, source_component: ComponentId) -> Self {
        self.source_component = Some(source_component);
        self
    }

    pub(crate) fn source_component(self) -> Option<ComponentId> {
        self.source_component
    }

    pub fn with_shade_color(mut self, color: [f32; 3]) -> Self {
        self.shade_color = sanitize_color(color, Self::DEFAULT_SHADE_COLOR);
        self
    }

    pub fn with_shade_strength(mut self, strength: f32) -> Self {
        self.shade_strength = sanitize_unit(strength, Self::DEFAULT_SHADE_STRENGTH);
        self
    }

    pub fn with_shade_threshold(mut self, threshold: f32) -> Self {
        self.shade_threshold = sanitize_nonnegative(threshold, Self::DEFAULT_SHADE_THRESHOLD);
        self.lit_threshold = self.lit_threshold.max(self.shade_threshold);
        self
    }

    pub fn with_lit_threshold(mut self, threshold: f32) -> Self {
        self.lit_threshold = sanitize_nonnegative(threshold, Self::DEFAULT_LIT_THRESHOLD);
        self.shade_threshold = self.shade_threshold.min(self.lit_threshold);
        self
    }

    pub fn with_rim_color(mut self, color: [f32; 3]) -> Self {
        self.rim_color = sanitize_color(color, Self::DEFAULT_RIM_COLOR);
        self
    }

    pub fn with_rim_strength(mut self, strength: f32) -> Self {
        self.rim_strength = sanitize_unit(strength, Self::DEFAULT_RIM_STRENGTH);
        self
    }

    pub fn with_rim_power(mut self, power: f32) -> Self {
        self.rim_power = if power.is_finite() {
            power.clamp(0.01, 128.0)
        } else {
            Self::DEFAULT_RIM_POWER
        };
        self
    }

    pub fn with_lighting_debug(mut self, mode: AnimeLightingDebug) -> Self {
        self.lighting_debug = mode;
        self
    }

    pub fn gpu_params(self) -> crate::engine::graphics::visual_world::AnimeShadingParams {
        crate::engine::graphics::visual_world::AnimeShadingParams {
            shade_color_strength: [
                self.shade_color[0],
                self.shade_color[1],
                self.shade_color[2],
                self.shade_strength,
            ],
            rim_color: [
                self.rim_color[0],
                self.rim_color[1],
                self.rim_color[2],
                self.lighting_debug.gpu_value(),
            ],
            controls: [
                self.shade_threshold,
                self.lit_threshold,
                self.rim_strength,
                self.rim_power,
            ],
        }
    }
}

impl Default for ShadingComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ShadingComponent {
    fn name(&self) -> &'static str {
        "shading"
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
            IntentValue::RegisterAnimeShading {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;

        if self.model == ShadingModel::Toon {
            return ce("Shading").with_call("toon", vec![]);
        }
        let mut node = ce("Shading")
            .with_call("anime", vec![])
            .with_call(
                "shade_color",
                vec![array(self.shade_color.map(|v| num(v as f64)).to_vec())],
            )
            .with_call("shade_strength", vec![num(self.shade_strength as f64)])
            .with_call("shade_threshold", vec![num(self.shade_threshold as f64)])
            .with_call("lit_threshold", vec![num(self.lit_threshold as f64)])
            .with_call(
                "rim_color",
                vec![array(self.rim_color.map(|v| num(v as f64)).to_vec())],
            )
            .with_call("rim_strength", vec![num(self.rim_strength as f64)])
            .with_call("rim_power", vec![num(self.rim_power as f64)]);
        if self.lighting_debug != AnimeLightingDebug::Anime {
            node = node.with_call("lighting_debug", vec![s(self.lighting_debug.name())]);
        }
        node
    }
}

fn sanitize_unit(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn sanitize_nonnegative(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        fallback
    }
}

fn sanitize_color(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    if value.iter().all(|channel| channel.is_finite()) {
        value.map(|channel| channel.clamp(0.0, 1.0))
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sanitizes_values_and_preserves_threshold_order() {
        let shading = AnimeShadingComponent::new()
            .with_shade_strength(2.0)
            .with_shade_threshold(0.8)
            .with_lit_threshold(0.4)
            .with_rim_power(f32::NAN);

        assert_eq!(shading.shade_strength, 1.0);
        assert_eq!(shading.shade_threshold, 0.4);
        assert_eq!(shading.lit_threshold, 0.4);
        assert_eq!(shading.rim_power, AnimeShadingComponent::DEFAULT_RIM_POWER);
    }
}
