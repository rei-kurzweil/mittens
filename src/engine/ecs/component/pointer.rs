use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// User-facing pointer component.
///
/// Attach this under the pose-driving part of the topology (for example a desktop camera rig
/// transform, an XR camera, or a controller-driven transform). At init time the engine spawns
/// and owns a child `RayCastComponent`, so authoring only needs to describe the pointer itself.
#[derive(Debug, Clone)]
pub struct PointerComponent {
    /// Explicit grip frame for Grabbable.to_hand objects. AVC supplies a runtime
    /// default when this override is absent or unresolved.
    pub grab_attachment: Option<crate::engine::ecs::component::ComponentRef>,
    pub(crate) avatar_grab_attachment: Option<ComponentId>,
    pub enabled: bool,
    /// Enables diagnostics owned by this pointer, currently its active drag mapping surface.
    pub debug_enabled: bool,
    /// Override for the clearance between a held object's ray-facing surface and pointer origin.
    pub min_grab_distance: Option<f32>,
    /// Maximum desktop cursor displacement that can still qualify as a click.
    pub click_max_screen_distance_px: f32,
    /// Maximum spatial pointer ray-direction change that can still qualify as a click.
    pub click_max_ray_angle_deg: f32,
    /// Maximum spatial pointer-origin displacement that can still qualify as a click, in metres.
    pub click_max_origin_distance: f32,

    component: Option<ComponentId>,
}

impl PointerComponent {
    pub const DEFAULT_CLICK_MAX_SCREEN_DISTANCE_PX: f32 = 8.0;
    pub const DEFAULT_CLICK_MAX_RAY_ANGLE_DEG: f32 = 2.0;
    pub const DEFAULT_CLICK_MAX_ORIGIN_DISTANCE: f32 = 0.03;

    pub fn new() -> Self {
        Self {
            grab_attachment: None,
            avatar_grab_attachment: None,
            enabled: true,
            debug_enabled: false,
            min_grab_distance: None,
            click_max_screen_distance_px: Self::DEFAULT_CLICK_MAX_SCREEN_DISTANCE_PX,
            click_max_ray_angle_deg: Self::DEFAULT_CLICK_MAX_RAY_ANGLE_DEG,
            click_max_origin_distance: Self::DEFAULT_CLICK_MAX_ORIGIN_DISTANCE,
            component: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            grab_attachment: None,
            avatar_grab_attachment: None,
            enabled: false,
            debug_enabled: false,
            min_grab_distance: None,
            click_max_screen_distance_px: Self::DEFAULT_CLICK_MAX_SCREEN_DISTANCE_PX,
            click_max_ray_angle_deg: Self::DEFAULT_CLICK_MAX_RAY_ANGLE_DEG,
            click_max_origin_distance: Self::DEFAULT_CLICK_MAX_ORIGIN_DISTANCE,
            component: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Enable or disable pointer-owned diagnostics.
    pub fn debug_enable(mut self, enabled: bool) -> Self {
        self.debug_enabled = enabled;
        self
    }

    pub fn min_grab_distance(mut self, meters: f32) -> Self {
        assert!(
            meters.is_finite() && meters >= 0.0,
            "minimum grab distance must be finite and non-negative"
        );
        self.min_grab_distance = Some(meters);
        self
    }

    pub fn click_max_screen_distance_px(mut self, px: f32) -> Self {
        assert!(
            px.is_finite() && px >= 0.0,
            "maximum click screen distance must be finite and non-negative"
        );
        self.click_max_screen_distance_px = px;
        self
    }

    pub fn click_max_ray_angle_deg(mut self, degrees: f32) -> Self {
        assert!(
            degrees.is_finite() && (0.0..=180.0).contains(&degrees),
            "maximum click ray angle must be finite and between 0 and 180 degrees"
        );
        self.click_max_ray_angle_deg = degrees;
        self
    }

    pub fn click_max_origin_distance(mut self, metres: f32) -> Self {
        assert!(
            metres.is_finite() && metres >= 0.0,
            "maximum click origin distance must be finite and non-negative"
        );
        self.click_max_origin_distance = metres;
        self
    }
}

impl Default for PointerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PointerComponent {
    fn name(&self) -> &'static str {
        "pointer"
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

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        self.component = Some(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterPointer {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let expression = if self.enabled {
            ce("Pointer")
        } else {
            ce_call("Pointer", "disabled", vec![])
        };
        let expression = if self.debug_enabled {
            expression.with_call("debug_enable", vec![b(true)])
        } else {
            expression
        };
        let expression = match &self.grab_attachment {
            Some(reference) => {
                use crate::engine::ecs::component::ComponentRef;
                let selector = match reference {
                    ComponentRef::Guid(id) => format!("@uuid:{id}"),
                    ComponentRef::Query(query) => query.clone(),
                };
                expression.with_call("grab_attachment", vec![s(&selector)])
            }
            None => expression,
        };
        let expression = match self.min_grab_distance {
            Some(distance) => expression.with_call("min_grab_distance", vec![num(distance as f64)]),
            None => expression,
        };
        let expression =
            if self.click_max_screen_distance_px != Self::DEFAULT_CLICK_MAX_SCREEN_DISTANCE_PX {
                expression.with_call(
                    "click_max_screen_distance_px",
                    vec![num(self.click_max_screen_distance_px as f64)],
                )
            } else {
                expression
            };
        let expression = if self.click_max_ray_angle_deg != Self::DEFAULT_CLICK_MAX_RAY_ANGLE_DEG {
            expression.with_call(
                "click_max_ray_angle_deg",
                vec![num(self.click_max_ray_angle_deg as f64)],
            )
        } else {
            expression
        };
        if self.click_max_origin_distance != Self::DEFAULT_CLICK_MAX_ORIGIN_DISTANCE {
            expression.with_call(
                "click_max_origin_distance",
                vec![num(self.click_max_origin_distance as f64)],
            )
        } else {
            expression
        }
    }
}
