use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

use crate::engine::ecs::system::model::collision_types::CollisionShape;

/// Explicit collision shape definition.
///
/// Intended to be added as a child of a `CollisionComponent`.
#[derive(Debug, Clone)]
pub struct CollisionShapeComponent {
    pub shape: CollisionShape,

    component: Option<ComponentId>,
}

impl CollisionShapeComponent {
    pub fn new(shape: CollisionShape) -> Self {
        Self {
            shape,
            component: None,
        }
    }

    pub fn cube() -> Self {
        Self::new(CollisionShape::CUBE())
    }

    pub fn sphere() -> Self {
        Self::new(CollisionShape::SPHERE())
    }
}

impl Component for CollisionShapeComponent {
    fn name(&self) -> &'static str {
        "collision_shape"
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
        match self.shape {
            CollisionShape::Cube { half_extents } => {
                map.insert("kind".to_string(), serde_json::json!("cube"));
                map.insert("half_extents".to_string(), serde_json::json!(half_extents));
            }
            CollisionShape::Sphere { radius } => {
                map.insert("kind".to_string(), serde_json::json!("sphere"));
                map.insert("radius".to_string(), serde_json::json!(radius));
            }
        }
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let Some(kind) = data.get("kind") else {
            return Ok(());
        };
        let kind_str: String = serde_json::from_value(kind.clone())
            .map_err(|e| format!("Failed to decode collision shape kind: {}", e))?;

        match kind_str.as_str() {
            "cube" => {
                if let Some(he) = data.get("half_extents") {
                    let half_extents: [f32; 3] = serde_json::from_value(he.clone())
                        .map_err(|e| format!("Failed to decode half_extents: {}", e))?;
                    self.shape = CollisionShape::cube_half_extents(half_extents);
                } else {
                    self.shape = CollisionShape::CUBE();
                }
            }
            "sphere" => {
                if let Some(r) = data.get("radius") {
                    let radius: f32 = serde_json::from_value(r.clone())
                        .map_err(|e| format!("Failed to decode radius: {}", e))?;
                    self.shape = CollisionShape::sphere_radius(radius);
                } else {
                    self.shape = CollisionShape::SPHERE();
                }
            }
            other => return Err(format!("Unknown collision shape kind: {}", other)),
        }

        Ok(())
    }
}
