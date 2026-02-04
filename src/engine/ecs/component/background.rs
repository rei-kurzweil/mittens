use crate::engine::ecs::component::Component;

#[derive(Debug, Default, Clone, Copy)]
pub struct BackgroundComponent {
    /// If true, renderables under this node render in the background *occluded+lit* stage.
    ///
    /// This stage is intended to depth-test/write against itself (for self-occlusion) and
    /// participate in lighting, while still not occluding the foreground (the renderer clears
    /// depth before drawing the foreground).
    pub occlusion_and_lighting: bool,
}

impl BackgroundComponent {
    pub fn new() -> Self {
        Self {
            occlusion_and_lighting: false,
        }
    }

    pub fn with_occlusion_and_lighting(mut self) -> Self {
        self.occlusion_and_lighting = true;
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
        Ok(())
    }
}
