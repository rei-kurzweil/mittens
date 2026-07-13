use super::Component;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};
use crate::engine::graphics::primitives::Transform;
use crate::utils::math::{
    mat_to_quat, quat_normalize, shortest_arc_quat, vec3_cross, vec3_dot, vec3_len, vec3_normalize,
    vec3_scale, vec3_sub,
};

#[derive(Debug, Clone, Copy)]
pub struct TransformComponent {
    /// Engine-wide transform type (also used by renderer/VisualWorld).
    pub transform: Transform,

    pub pending_look_at_target_world: Option<[f32; 3]>,

    component: Option<ComponentId>,
}

impl TransformComponent {
    pub fn new() -> Self {
        let transform = Transform::default();
        Self {
            transform,
            pending_look_at_target_world: None,
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

    pub fn with_looking_at(mut self, target_world: [f32; 3]) -> Self {
        self.pending_look_at_target_world = Some(target_world);
        self
    }

    pub fn look_at_world_rotation(
        world_position: [f32; 3],
        target_world: [f32; 3],
    ) -> Option<[f32; 4]> {
        let forward = vec3_sub(target_world, world_position);
        if vec3_len(forward) <= 1e-5 {
            return None;
        }

        let z = vec3_normalize(forward);
        let fallback_up = if z[1].abs() < 0.99 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let projected_up = vec3_sub(fallback_up, vec3_scale(z, vec3_dot(fallback_up, z)));
        let y = if vec3_len(projected_up) > 1e-5 {
            vec3_normalize(projected_up)
        } else {
            fallback_up
        };
        let x = vec3_normalize(vec3_cross(y, z));
        let y = vec3_normalize(vec3_cross(z, x));

        let basis = [
            [x[0], x[1], x[2], 0.0],
            [y[0], y[1], y[2], 0.0],
            [z[0], z[1], z[2], 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let quat = quat_normalize(mat_to_quat(basis));
        Some(if quat == [0.0, 0.0, 0.0, 1.0] && z != [0.0, 0.0, 1.0] {
            shortest_arc_quat([0.0, 0.0, 1.0], z)
        } else {
            quat
        })
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
        emit.push_intent_now(
            cid,
            IntentValue::UpdateTransform {
                component_ids: vec![cid],
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
        emit.push_intent_now(
            cid,
            IntentValue::UpdateTransform {
                component_ids: vec![cid],
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
        emit.push_intent_now(
            cid,
            IntentValue::UpdateTransform {
                component_ids: vec![cid],
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
        emit.push_intent_now(
            cid,
            IntentValue::UpdateTransform {
                component_ids: vec![cid],
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
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterTransform {
                component_ids: vec![component],
            },
        );
        if let Some(target_world) = self.pending_look_at_target_world.take() {
            emit.push_intent_now(
                component,
                crate::engine::ecs::IntentValue::LookAt {
                    component_ids: vec![component],
                    target_world,
                },
            );
        }
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let t = &self.transform;
        // Emit position, rotation_quat (lossless), scale — matches the
        // builder vocabulary in `component_registry::apply_transform_builder`.
        ce_call(
            "Transform",
            "position",
            nums(t.translation.iter().map(|&v| v as f64)),
        )
        .with_call("rotation_quat", nums(t.rotation.iter().map(|&v| v as f64)))
        .with_call("scale", nums(t.scale.iter().map(|&v| v as f64)))
    }
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::TransformComponent;
    use crate::engine::ecs::system::TransformSystem;
    use crate::engine::ecs::{CommandQueue, IntentValue, SignalEmitter, SystemWorld, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use crate::utils::math::{mat_to_quat, quat_rotate_vec3, vec3_dot, vec3_len, vec3_normalize};

    fn flush(
        world: &mut World,
        systems: &mut SystemWorld,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        queue: &mut CommandQueue,
    ) {
        queue.flush(world, systems, visuals, render_assets);
    }

    fn world_forward(world: &World, cid: crate::engine::ecs::ComponentId) -> [f32; 3] {
        let world_rot = mat_to_quat(TransformSystem::world_model(world, cid).expect("world model"));
        vec3_normalize(quat_rotate_vec3(world_rot, [0.0, 0.0, 1.0]))
    }

    fn assert_faces_target(world: &World, cid: crate::engine::ecs::ComponentId, target: [f32; 3]) {
        let position = TransformSystem::world_position(world, cid).expect("world position");
        let desired = vec3_normalize([
            target[0] - position[0],
            target[1] - position[1],
            target[2] - position[2],
        ]);
        let actual = world_forward(world, cid);
        assert!(
            vec3_dot(actual, desired) > 0.999,
            "expected forward {:?} to face {:?}",
            actual,
            desired
        );
    }

    #[test]
    fn look_at_executor_preserves_translation_and_scale() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut queue = CommandQueue::new();

        let root = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 2.0, 3.0)
                .with_scale(2.0, 3.0, 4.0),
        );
        world.init_component_tree(root, &mut queue);
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        queue.push_intent_now(
            root,
            IntentValue::LookAt {
                component_ids: vec![root],
                target_world: [1.0, 2.0, 8.0],
            },
        );
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        let transform = world
            .get_component_by_id_as::<TransformComponent>(root)
            .expect("transform");
        assert_eq!(transform.transform.translation, [1.0, 2.0, 3.0]);
        assert_eq!(transform.transform.scale, [2.0, 3.0, 4.0]);
        assert_faces_target(&world, root, [1.0, 2.0, 8.0]);
    }

    #[test]
    fn look_at_executor_respects_rotated_parent_world_space_target() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut queue = CommandQueue::new();

        let parent =
            world.add_component(TransformComponent::new().with_rotation_euler(0.0, 1.1, 0.0));
        let child = world.add_component(TransformComponent::new().with_position(0.0, 0.0, 2.0));
        world.add_child(parent, child).unwrap();
        world.init_component_tree(parent, &mut queue);
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        let target = [4.0, 1.0, -3.0];
        queue.push_intent_now(
            child,
            IntentValue::LookAt {
                component_ids: vec![child],
                target_world: target,
            },
        );
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        assert_faces_target(&world, child, target);
    }

    #[test]
    fn authored_pending_look_at_is_order_independent() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut queue = CommandQueue::new();

        let a = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 0.0, 0.0)
                .with_looking_at([3.0, 0.5, 1.0]),
        );
        let b = world.add_component(
            TransformComponent::new()
                .with_looking_at([3.0, 0.5, 1.0])
                .with_position(1.0, 0.0, 0.0),
        );
        world.init_component_tree(a, &mut queue);
        world.init_component_tree(b, &mut queue);
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        let rot_a = world
            .get_component_by_id_as::<TransformComponent>(a)
            .expect("a")
            .transform
            .rotation;
        let rot_b = world
            .get_component_by_id_as::<TransformComponent>(b)
            .expect("b")
            .transform
            .rotation;
        let dot =
            rot_a[0] * rot_b[0] + rot_a[1] * rot_b[1] + rot_a[2] * rot_b[2] + rot_a[3] * rot_b[3];
        assert!(
            dot.abs() > 0.9999,
            "expected equivalent rotations, got {:?} vs {:?}",
            rot_a,
            rot_b
        );
    }

    #[test]
    fn look_at_degenerate_and_collinear_cases_are_stable() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut queue = CommandQueue::new();

        let same_target = world.add_component(
            TransformComponent::new()
                .with_position(2.0, 3.0, 4.0)
                .with_rotation_quat([0.0, 0.70710677, 0.0, 0.70710677]),
        );
        let vertical_target = world.add_component(TransformComponent::new());
        world.init_component_tree(same_target, &mut queue);
        world.init_component_tree(vertical_target, &mut queue);
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        let original = world
            .get_component_by_id_as::<TransformComponent>(same_target)
            .expect("same_target")
            .transform
            .rotation;
        queue.push_intent_now(
            same_target,
            IntentValue::LookAt {
                component_ids: vec![same_target],
                target_world: [2.0, 3.0, 4.0],
            },
        );
        queue.push_intent_now(
            vertical_target,
            IntentValue::LookAt {
                component_ids: vec![vertical_target],
                target_world: [0.0, 5.0, 0.0],
            },
        );
        flush(
            &mut world,
            &mut systems,
            &mut visuals,
            &mut render_assets,
            &mut queue,
        );

        let same_rotation = world
            .get_component_by_id_as::<TransformComponent>(same_target)
            .expect("same_target")
            .transform
            .rotation;
        assert_eq!(same_rotation, original);

        let vertical_rotation = world
            .get_component_by_id_as::<TransformComponent>(vertical_target)
            .expect("vertical_target")
            .transform
            .rotation;
        assert!(vertical_rotation.iter().all(|v| v.is_finite()));
        assert!(vec3_len(world_forward(&world, vertical_target)) > 0.99);
        assert_faces_target(&world, vertical_target, [0.0, 5.0, 0.0]);
    }
}
