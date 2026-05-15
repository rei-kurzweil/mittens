use crate::engine::ecs::component::Component;

#[derive(Debug, Default, Clone, Copy)]
pub struct BackgroundComponent {
    /// If true, renderables under this node render in the background *occluded+lit* stage.
    ///
    /// This stage is intended to depth-test/write against itself (for self-occlusion) and
    /// participate in lighting, while still not occluding the foreground (the renderer clears
    /// depth before drawing the foreground).
    pub occlusion_and_lighting: bool,

    /// If true, renderables under this node are eligible for ray casting (BVH insertion).
    ///
    /// Default is false because background scene dressing (clouds, skyboxes, etc.) typically
    /// should not be hit-testable.
    pub ray_casting: bool,
}

impl BackgroundComponent {
    pub fn new() -> Self {
        Self {
            occlusion_and_lighting: false,
            ray_casting: false,
        }
    }

    pub fn with_occlusion_and_lighting(mut self) -> Self {
        self.occlusion_and_lighting = true;
        self
    }

    pub fn with_ray_casting(mut self) -> Self {
        self.ray_casting = true;
        self
    }
}

impl Component for BackgroundComponent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "background"
    }

    fn to_mms_ast(&self) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce("Background");
        if self.occlusion_and_lighting {
            ce = ce.with_call("occlusion_and_lighting", vec![]);
        }
        if self.ray_casting {
            ce = ce.with_call("ray_casting", vec![]);
        }
        ce
    }
}
