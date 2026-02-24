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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "occlusion_and_lighting".to_string(),
            serde_json::json!(self.occlusion_and_lighting),
        );
        map.insert(
            "ray_casting".to_string(),
            serde_json::json!(self.ray_casting),
        );
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("occlusion_and_lighting") {
            if let Some(b) = v.as_bool() {
                self.occlusion_and_lighting = b;
            }
        }
        if let Some(v) = data.get("ray_casting") {
            if let Some(b) = v.as_bool() {
                self.ray_casting = b;
            }
        }
        Ok(())
    }
}
