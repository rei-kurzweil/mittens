use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureCoordType {
    WorldPlane,
    ScreenSpace1DSlider,
}

#[derive(Debug, Clone, Copy)]
pub struct GestureCoordTypeComponent {
    pub coord_type: GestureCoordType,

    component: Option<ComponentId>,
}

impl GestureCoordTypeComponent {
    pub fn new(coord_type: GestureCoordType) -> Self {
        Self {
            coord_type,
            component: None,
        }
    }

    pub fn world_plane() -> Self {
        Self::new(GestureCoordType::WorldPlane)
    }

    pub fn screen_space_1d_slider() -> Self {
        Self::new(GestureCoordType::ScreenSpace1DSlider)
    }
}

impl Default for GestureCoordTypeComponent {
    fn default() -> Self {
        Self::world_plane()
    }
}

impl Component for GestureCoordTypeComponent {
    fn name(&self) -> &'static str {
        "gesture_coord_type"
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

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        let coord_type = match self.coord_type {
            GestureCoordType::WorldPlane => "world_plane",
            GestureCoordType::ScreenSpace1DSlider => "screen_space_1d_slider",
        };
        map.insert("coord_type".to_string(), serde_json::json!(coord_type));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(v) = data.get("coord_type") {
            let s: String = serde_json::from_value(v.clone())
                .map_err(|e| format!("Failed to decode coord_type: {}", e))?;
            self.coord_type = match s.as_str() {
                "world_plane" => GestureCoordType::WorldPlane,
                "screen_space_1d_slider" => GestureCoordType::ScreenSpace1DSlider,
                other => return Err(format!("Unknown coord_type: {}", other)),
            };
        }
        Ok(())
    }
}
