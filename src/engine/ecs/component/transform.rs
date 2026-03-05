use super::Component;
use crate::engine::ecs::{ComponentId, SignalEmitter, SignalValue};
use crate::engine::graphics::primitives::Transform;

#[derive(Debug, Clone, Copy)]
pub struct TransformComponent {
    /// Engine-wide transform type (also used by renderer/VisualWorld).
    pub transform: Transform,

    component: Option<ComponentId>,
}

impl TransformComponent {
    pub fn new() -> Self {
        let transform = Transform::default();
        Self {
            transform,
            component: None,
        }
    }

    fn recompute_model(&mut self) {
        self.transform.recompute_model();
    }

    pub fn with_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.transform.translation = [x, y, z];
        self.recompute_model();
        self
    }

    pub fn with_scale(mut self, x: f32, y: f32, z: f32) -> Self {
        self.transform.scale = [x, y, z];
        self.recompute_model();
        self
    }

    /// Builder-style: set rotation from Euler angles (radians), returns Self.
    pub fn with_rotation_euler(mut self, pitch_x: f32, yaw_y: f32, roll_z: f32) -> Self {
        self.set_rotation_euler_internal(pitch_x, yaw_y, roll_z);
        self
    }

    /// Builder-style: set rotation from a quaternion (xyzw), returns Self.
    pub fn with_rotation_quat(mut self, quat_xyzw: [f32; 4]) -> Self {
        self.set_rotation_quat_internal(quat_xyzw);
        self
    }

    /// Private helper: computes and sets quaternion from euler angles, then recomputes model.
    fn set_rotation_euler_internal(&mut self, pitch_x: f32, yaw_y: f32, roll_z: f32) {
        // Minimal Euler->quat (XYZ intrinsic) implementation.
        let (sx, cx) = (0.5 * pitch_x).sin_cos();
        let (sy, cy) = (0.5 * yaw_y).sin_cos();
        let (sz, cz) = (0.5 * roll_z).sin_cos();

        // q = qx * qy * qz
        let qx = [sx, 0.0, 0.0, cx];
        let qy = [0.0, sy, 0.0, cy];
        let qz = [0.0, 0.0, sz, cz];

        fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
            let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
            let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
            [
                aw * bx + ax * bw + ay * bz - az * by,
                aw * by - ax * bz + ay * bw + az * bx,
                aw * bz + ax * by - ay * bx + az * bw,
                aw * bw - ax * bx - ay * by - az * bz,
            ]
        }

        let qxy = quat_mul(qx, qy);
        let q = quat_mul(qxy, qz);
        self.transform.rotation = q;
        self.recompute_model();
    }

    /// Private helper: sets quaternion directly, then recomputes model.
    fn set_rotation_quat_internal(&mut self, quat_xyzw: [f32; 4]) {
        self.transform.rotation = quat_xyzw;
        self.recompute_model();
    }

    /// Set rotation from Euler angles (radians), XYZ order, and queue update.
    pub fn set_rotation_euler(
        &mut self,
        emit: &mut dyn SignalEmitter,
        pitch_x: f32,
        yaw_y: f32,
        roll_z: f32,
    ) {
        self.set_rotation_euler_internal(pitch_x, yaw_y, roll_z);

        let Some(cid) = self.component else {
            return;
        };
        emit.push(
            cid,
            SignalValue::UpdateTransform {
                component: cid,
                translation: self.transform.translation,
                rotation_quat_xyzw: self.transform.rotation,
                scale: self.transform.scale,
            },
        );
    }

    /// Set rotation from a quaternion (xyzw) and queue update.
    pub fn set_rotation_quat(&mut self, emit: &mut dyn SignalEmitter, quat_xyzw: [f32; 4]) {
        self.set_rotation_quat_internal(quat_xyzw);

        let Some(cid) = self.component else {
            return;
        };
        emit.push(
            cid,
            SignalValue::UpdateTransform {
                component: cid,
                translation: self.transform.translation,
                rotation_quat_xyzw: self.transform.rotation,
                scale: self.transform.scale,
            },
        );
    }

    /// Set translation and queue update.
    pub fn set_position(&mut self, emit: &mut dyn SignalEmitter, x: f32, y: f32, z: f32) {
        self.transform.translation = [x, y, z];
        self.recompute_model();
        let Some(cid) = self.component else {
            return;
        };
        emit.push(
            cid,
            SignalValue::UpdateTransform {
                component: cid,
                translation: self.transform.translation,
                rotation_quat_xyzw: self.transform.rotation,
                scale: self.transform.scale,
            },
        );
    }

    /// Set non-uniform scale and queue update.
    pub fn set_scale(&mut self, emit: &mut dyn SignalEmitter, x: f32, y: f32, z: f32) {
        self.transform.scale = [x, y, z];
        self.recompute_model();
        let Some(cid) = self.component else {
            return;
        };
        emit.push(
            cid,
            SignalValue::UpdateTransform {
                component: cid,
                translation: self.transform.translation,
                rotation_quat_xyzw: self.transform.rotation,
                scale: self.transform.scale,
            },
        );
    }
}

impl Component for TransformComponent {
    fn name(&self) -> &'static str {
        "transform"
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
        emit.push(
            component,
            crate::engine::ecs::SignalValue::RegisterTransform { component },
        );
    }

    fn encode(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut map = std::collections::HashMap::new();
        map.insert("model".to_string(), serde_json::json!(self.transform.model));
        map
    }

    fn decode(
        &mut self,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(model) = data.get("model") {
            self.transform.model = serde_json::from_value(model.clone())
                .map_err(|e| format!("Failed to decode model matrix: {}", e))?;
            // Keep derived state in a sane starting point; TransformSystem will recompute.
            self.transform.matrix_world = self.transform.model;
        }
        Ok(())
    }
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self::new()
    }
}
