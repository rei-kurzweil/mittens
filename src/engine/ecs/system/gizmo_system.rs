use crate::engine::ecs::component::{
    GestureCoordType, GestureCoordTypeComponent, TransformComponent, TransformGizmoAxis,
    TransformGizmoComponent, TransformGizmoRotateComponent, TransformGizmoScaleComponent,
    TransformGizmoTranslateComponent,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::engine::user_input::InputState;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy)]
enum TransformGizmoOp {
    Translate(TransformGizmoAxis),
    Rotate(TransformGizmoAxis),
    Scale(TransformGizmoAxis),
}

#[derive(Debug, Default)]
pub struct TransformGizmoSystem;

impl TransformGizmoSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install per-gizmo scoped handlers rooted at the `TransformGizmoComponent` node.
    ///
    /// Drag events are scoped to the hit renderable; because gizmo handle renderables live under
    /// the gizmo node, scoped handlers rooted at the gizmo will run for drag events on its handles.
    pub fn install_scoped_handlers_for_gizmo(&mut self, rx: &mut RxWorld, gizmo_root: ComponentId) {
        rx.add_handler(SignalKind::ParentChanged, gizmo_root, Self::on_parent_changed);
        rx.add_handler(SignalKind::DragStart, gizmo_root, Self::on_drag_start);
        rx.add_handler(SignalKind::DragMove, gizmo_root, Self::on_drag_move);
        rx.add_handler(SignalKind::DragEnd, gizmo_root, Self::on_drag_end);
    }

    fn debug_drag_plane_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GIZMO_DRAG_PLANE").unwrap_or_default();
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
    }

    fn debug_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GIZMO").unwrap_or_default();
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
    }

    fn quat_from_z_to_dir(dir: [f32; 3]) -> [f32; 4] {
        use crate::utils::math;

        fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }

        // Rotate local +Z to `dir`.
        let z = [0.0f32, 0.0f32, 1.0f32];
        let d = math::vec3_normalize(dir);
        let dot_ = z[0] * d[0] + z[1] * d[1] + z[2] * d[2];

        if dot_ >= 1.0 - 1e-6 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        if dot_ <= -1.0 + 1e-6 {
            // 180-degree flip around X (any axis orthogonal to Z works).
            return math::quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::PI);
        }

        let axis = cross(z, d);
        let axis_len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if axis_len <= 1e-6 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        let axis_n = [axis[0] / axis_len, axis[1] / axis_len, axis[2] / axis_len];
        let angle = dot_.clamp(-1.0, 1.0).acos();
        math::quat_from_axis_angle(axis_n, angle)
    }

    fn spawn_debug_drag_plane(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        hit_point: [f32; 3],
        plane_normal: [f32; 3],
    ) -> ComponentId {
        use crate::engine::ecs::component::{
            ColorComponent, EmissiveComponent, OpacityComponent, RenderableComponent,
            TransformComponent,
        };
        use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle, Renderable};

        let q = Self::quat_from_z_to_dir(plane_normal);

        // Use a very thin cube so it is visible from both sides (debug aid).
        let size = 2.0_f32;
        let thickness = 0.005_f32;

        let t = world.add_component_boxed_named(
            "gizmo_drag_plane_t",
            Box::new(
                TransformComponent::new()
                    .with_position(hit_point[0], hit_point[1], hit_point[2])
                    .with_rotation_quat(q)
                    .with_scale(size, size, thickness),
            ),
        );
        let r = world.add_component_boxed_named(
            "gizmo_drag_plane_r",
            Box::new(RenderableComponent::new(Renderable::new(
                CpuMeshHandle::CUBE,
                MaterialHandle::UNLIT_MESH,
            ))),
        );
        let c = world.add_component_boxed_named(
            "gizmo_drag_plane_color",
            Box::new(ColorComponent::rgba(1.0, 0.0, 1.0, 0.35)),
        );
        let o = world.add_component_boxed_named(
            "gizmo_drag_plane_opacity",
            Box::new(
                OpacityComponent::new()
                    .with_opacity(0.35)
                    .with_multiple_layers(),
            ),
        );
        let e = world.add_component_boxed_named(
            "gizmo_drag_plane_emissive",
            Box::new(EmissiveComponent::on()),
        );

        let _ = world.add_child(t, r);
        let _ = world.add_child(r, c);
        let _ = world.add_child(r, o);
        let _ = world.add_child(r, e);

        world.init_component_tree(t, emit);
        t
    }

    fn on_parent_changed(world: &mut World, _emit: &mut dyn SignalEmitter, env: &crate::engine::ecs::Signal) {
        let Some(EventSignal::ParentChanged {
            child,
            new_parent,
            ..
        }) = env.event.as_ref()
        else {
            return;
        };

        if world
            .get_component_by_id_as::<TransformGizmoComponent>(*child)
            .is_none()
        {
            return;
        }

        let mut target: Option<ComponentId> = None;
        let mut cur = *new_parent;
        while let Some(node) = cur {
            if world.get_component_by_id_as::<TransformComponent>(node).is_some() {
                target = Some(node);
                break;
            }
            cur = world.parent_of(node);
        }

        let old_target = world
            .get_component_by_id_as::<TransformGizmoComponent>(*child)
            .and_then(|g| g.target_transform);

        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(*child) {
            g.target_transform = target;
            g.active_raycaster = None;
        }

        if Self::debug_enabled() {
            println!(
                "[TransformGizmoSystem] ParentChanged gizmo={:?} new_parent={:?} old_target={:?} new_target={:?}",
                child, new_parent, old_target, target
            );
        }
    }

    fn on_drag_start(world: &mut World, emit: &mut dyn SignalEmitter, env: &crate::engine::ecs::Signal) {
        let Some(EventSignal::DragStart {
            raycaster,
            renderable,
            hit_point,
            ray_dir_world,
            ..
        }) = env.event.as_ref()
        else {
            return;
        };

        let Some((gizmo_cid, _op)) = Self::resolve_gizmo_op_for_renderable(world, *renderable) else {
            return;
        };

        let mut old_debug_root: Option<ComponentId> = None;
        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid) {
            g.active_raycaster = Some(*raycaster);
            if Self::debug_drag_plane_enabled() {
                old_debug_root = g.debug_drag_plane_root.take();
            }
        }

        if let Some(root) = old_debug_root {
            emit.push_intent_now(root, IntentValue::RemoveSubtree { target: vec![root] });
        }

        if Self::debug_drag_plane_enabled() {
            let plane_root = Self::spawn_debug_drag_plane(world, emit, *hit_point, *ray_dir_world);
            if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid) {
                g.debug_drag_plane_root = Some(plane_root);
            }
        }
    }

    fn on_drag_move(world: &mut World, emit: &mut dyn SignalEmitter, env: &crate::engine::ecs::Signal) {
        use crate::engine::ecs::system::transform_system::TransformSystem;
        use crate::utils::math;

        fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
        }

        fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
        }

        fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
        }

        fn mul(v: [f32; 3], s: f32) -> [f32; 3] {
            [v[0] * s, v[1] * s, v[2] * s]
        }

        fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }

        let Some(EventSignal::DragMove {
            raycaster,
            renderable,
            delta_world,
            hit_point,
            screen_delta_px,
            ..
        }) = env.event.as_ref()
        else {
            return;
        };

        let Some((gizmo_cid, op)) = Self::resolve_gizmo_op_for_renderable(world, *renderable) else {
            return;
        };

        // Copy out what we need without holding a mutable borrow.
        let Some((target_transform, active)) = world
            .get_component_by_id_as::<TransformGizmoComponent>(gizmo_cid)
            .map(|g| (g.target_transform, g.active_raycaster))
        else {
            return;
        };

        let Some(target_transform) = target_transform else {
            return;
        };

        if active != Some(*raycaster) {
            return;
        }

        match op {
            TransformGizmoOp::Translate(axis) => {
                let axis_v = axis.unit_vec3();
                let d = dot(*delta_world, axis_v);
                let delta = mul(axis_v, d);

                let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target_transform) else {
                    return;
                };

                let cur = t.transform.translation;
                let next = add(cur, delta);
                t.set_position(emit, next[0], next[1], next[2]);
            }
            TransformGizmoOp::Rotate(axis) => {
                let coord_type = Self::resolve_gesture_coord_type_for_renderable(world, *renderable);

                let axis_v = axis.unit_vec3();
                let angle = match (coord_type, *screen_delta_px) {
                    (Some(GestureCoordType::ScreenSpace1DSlider), Some((dx, dy))) => {
                        // Simple first-pass slider mapping. We can refine sign selection later
                        // (camera-aware) without changing the signal.
                        let radians_per_px = 0.01_f32;
                        let px = match axis {
                            TransformGizmoAxis::X => -dy,
                            TransformGizmoAxis::Y => dx,
                            TransformGizmoAxis::Z => dx,
                        };
                        px * radians_per_px
                    }
                    _ => {
                        let pivot = TransformSystem::world_position(world, target_transform)
                            .unwrap_or([0.0, 0.0, 0.0]);
                        let prev_hit = sub(*hit_point, *delta_world);

                        let mut v0 = sub(prev_hit, pivot);
                        let mut v1 = sub(*hit_point, pivot);

                        // Project onto plane orthogonal to the axis.
                        v0 = sub(v0, mul(axis_v, dot(v0, axis_v)));
                        v1 = sub(v1, mul(axis_v, dot(v1, axis_v)));
                        v0 = math::vec3_normalize(v0);
                        v1 = math::vec3_normalize(v1);

                        // Signed angle about axis.
                        let c = cross(v0, v1);
                        let s = dot(axis_v, c);
                        let d = dot(v0, v1);
                        s.atan2(d)
                    }
                };

                if angle != 0.0 {
                    let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target_transform) else {
                        return;
                    };
                    let q_delta = math::quat_from_axis_angle(axis_v, angle);
                    let q_next = math::quat_mul(q_delta, t.transform.rotation);
                    t.set_rotation_quat(emit, q_next);
                }
            }
            TransformGizmoOp::Scale(axis) => {
                let d = dot(*delta_world, axis.unit_vec3());

                let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target_transform) else {
                    return;
                };

                let mut s = t.transform.scale;
                match axis {
                    TransformGizmoAxis::X => s[0] = (s[0] + d).max(0.001),
                    TransformGizmoAxis::Y => s[1] = (s[1] + d).max(0.001),
                    TransformGizmoAxis::Z => s[2] = (s[2] + d).max(0.001),
                }
                t.set_scale(emit, s[0], s[1], s[2]);
            }
        }
    }

    fn on_drag_end(world: &mut World, emit: &mut dyn SignalEmitter, env: &crate::engine::ecs::Signal) {
        let Some(EventSignal::DragEnd {
            raycaster,
            renderable,
            ..
        }) = env.event.as_ref()
        else {
            return;
        };

        let Some((gizmo_cid, _op)) = Self::resolve_gizmo_op_for_renderable(world, *renderable) else {
            return;
        };

        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid) {
            if g.active_raycaster == Some(*raycaster) {
                g.active_raycaster = None;
            }

            if Self::debug_drag_plane_enabled() {
                if let Some(root) = g.debug_drag_plane_root.take() {
                    emit.push_intent_now(root, IntentValue::RemoveSubtree { target: vec![root] });
                }
            }
        }
    }

    /// Spawn the 9-part gizmo visual subtree for a TransformGizmoComponent.
    ///
    /// Contract: TransformGizmoComponent is expected to be attached under a TransformComponent.
    pub fn register_transform_gizmo(
        &mut self,
        world: &mut World,
        component: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) {
        use crate::engine::ecs::component::{
            OverlayComponent, TransformComponent, TransformGizmoAxis, TransformGizmoComponent,
            TransformGizmoRotateComponent, TransformGizmoTranslateComponent,
        };
        use crate::engine::graphics::primitives::CpuMeshHandle;

        // Must be a gizmo.
        let Some(_) = world.get_component_by_id_as::<TransformGizmoComponent>(component) else {
            return;
        };

        // Find the nearest ancestor transform to attach visuals under.
        let mut cur = component;
        let mut parent_transform: Option<ComponentId> = None;
        while let Some(p) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<TransformComponent>(p)
                .is_some()
            {
                parent_transform = Some(p);
                break;
            }
            cur = p;
        }
        if parent_transform.is_none() {
            return;
        }
        let parent_transform = parent_transform.unwrap();

        // Bind gizmo target to the attached TransformComponent.
        // This is the only supported targeting mode (works for joints/armatures and normal transforms).
        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(component) {
            g.target_transform = Some(parent_transform);
        }

        // Avoid respawn.
        if let Some(g) = world.get_component_by_id_as::<TransformGizmoComponent>(component) {
            if g.visual_root.is_some() {
                return;
            }
        }

        let gizmo_scale = world
            .get_component_by_id_as::<TransformGizmoComponent>(component)
            .map(|g| g.scale)
            .unwrap_or(1.0);

        // Gizmos are parented under the target transform, so by default they'd inherit whatever
        // scale the target (and its ancestors) have. For joints/armatures this can make gizmos
        // extremely tiny.
        //
        // Interpret `TransformGizmoComponent.scale` as an intended *world-space* scale multiplier
        // and compensate for the target's current world scale when choosing the local scale for
        // the gizmo visual root.
        fn mat4_identity() -> crate::engine::graphics::primitives::TransformMatrix {
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        }

        fn mat4_mul(
            a: crate::engine::graphics::primitives::TransformMatrix,
            b: crate::engine::graphics::primitives::TransformMatrix,
        ) -> crate::engine::graphics::primitives::TransformMatrix {
            let mut out = [[0.0f32; 4]; 4];
            for c in 0..4 {
                for r in 0..4 {
                    out[c][r] = a[0][r] * b[c][0]
                        + a[1][r] * b[c][1]
                        + a[2][r] * b[c][2]
                        + a[3][r] * b[c][3];
                }
            }
            out
        }

        fn max_basis_scale(m: crate::engine::graphics::primitives::TransformMatrix) -> f32 {
            fn len3(v: [f32; 4]) -> f32 {
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
            }
            // Column-major: columns 0..2 are the scaled basis vectors.
            len3(m[0]).max(len3(m[1])).max(len3(m[2]))
        }

        fn world_model_uncached(
            world: &World,
            start_transform: ComponentId,
        ) -> crate::engine::graphics::primitives::TransformMatrix {
            // Collect local model matrices for all ancestor transforms, then multiply root->leaf.
            let mut chain: Vec<crate::engine::graphics::primitives::TransformMatrix> = Vec::new();
            let mut cur: Option<ComponentId> = Some(start_transform);
            while let Some(node) = cur {
                if let Some(t) = world.get_component_by_id_as::<TransformComponent>(node) {
                    chain.push(t.transform.model);
                }
                cur = world.parent_of(node);
            }
            chain.reverse();
            let mut out = mat4_identity();
            for m in chain {
                out = mat4_mul(out, m);
            }
            out
        }

        let parent_world = world_model_uncached(world, parent_transform);
        let parent_world_scale = max_basis_scale(parent_world).max(1e-4);
        let gizmo_local_scale = gizmo_scale / parent_world_scale;

        if Self::debug_enabled() {
            println!(
                "[TransformGizmoSystem] register gizmo={:?} target_transform={:?} requested_world_scale={:.4} parent_world_scale={:.4} gizmo_local_scale={:.4}",
                component,
                parent_transform,
                gizmo_scale,
                parent_world_scale,
                gizmo_local_scale
            );
        }

        // Create a root transform for the gizmo visuals under the GizmoComponent node.
        let gizmo_root = world.add_component_boxed_named(
            "gizmo_root",
            Box::new(
                TransformComponent::new().with_scale(
                    gizmo_local_scale,
                    gizmo_local_scale,
                    gizmo_local_scale,
                ),
            ),
        );
        let _ = world.add_child(component, gizmo_root);

        // Wrap all gizmo visuals in an overlay marker so they render in the overlay pass.
        let gizmo_overlay =
            world.add_component_boxed_named("gizmo_overlay", Box::new(OverlayComponent::new()));
        let _ = world.add_child(gizmo_root, gizmo_overlay);

        let gizmo_visual_parent = gizmo_overlay;

        // Write back visual root.
        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(component) {
            g.visual_root = Some(gizmo_root);
        }

        // Helper: spawn a renderable under a transform with color+emissive.
        fn spawn_part(
            world: &mut World,
            parent: ComponentId,
            name: &str,
            mesh: CpuMeshHandle,
            pos: [f32; 3],
            rot_euler: [f32; 3],
            scale: [f32; 3],
            rgba: [f32; 4],
        ) {
            use crate::engine::ecs::component::{
                ColorComponent, EmissiveComponent, RenderableComponent, TransformComponent,
            };
            use crate::engine::graphics::primitives::{MaterialHandle, Renderable};

            let t = world.add_component_boxed_named(
                format!("{name}_t"),
                Box::new(
                    TransformComponent::new()
                        .with_position(pos[0], pos[1], pos[2])
                        .with_rotation_euler(rot_euler[0], rot_euler[1], rot_euler[2])
                        .with_scale(scale[0], scale[1], scale[2]),
                ),
            );
            let r = world.add_component_boxed_named(
                format!("{name}_r"),
                Box::new(RenderableComponent::new(Renderable::new(
                    mesh,
                    MaterialHandle::TOON_MESH,
                ))),
            );
            let c = world.add_component_boxed_named(
                format!("{name}_color"),
                Box::new(ColorComponent::rgba(rgba[0], rgba[1], rgba[2], rgba[3])),
            );
            let e = world.add_component_boxed_named(
                format!("{name}_emissive"),
                Box::new(EmissiveComponent::on()),
            );

            let _ = world.add_child(parent, t);
            let _ = world.add_child(t, r);
            let _ = world.add_child(r, c);
            let _ = world.add_child(r, e);
        }

        // Helper: create a single raycastable root node for a handle subtree.
        // Descendant renderables become BVH-eligible via ancestry.
        fn spawn_raycastable_root(
            world: &mut World,
            parent: ComponentId,
            name: &str,
        ) -> ComponentId {
            use crate::engine::ecs::component::RaycastableComponent;

            let rc =
                world.add_component_boxed_named(name, Box::new(RaycastableComponent::enabled()));
            let _ = world.add_child(parent, rc);
            rc
        }

        fn spawn_gesture_coord_type_root(
            world: &mut World,
            parent: ComponentId,
            name: &str,
            coord_type: GestureCoordType,
        ) -> ComponentId {
            let c = world.add_component_boxed_named(
                name,
                Box::new(GestureCoordTypeComponent::new(coord_type)),
            );
            let _ = world.add_child(parent, c);
            c
        }

        fn spawn_translate_handle_root(
            world: &mut World,
            parent: ComponentId,
            axis: TransformGizmoAxis,
            name: &str,
        ) -> ComponentId {
            let h = world.add_component_boxed_named(
                name,
                Box::new(TransformGizmoTranslateComponent::new(axis)),
            );
            let _ = world.add_child(parent, h);
            h
        }

        fn spawn_rotate_handle_root(
            world: &mut World,
            parent: ComponentId,
            axis: TransformGizmoAxis,
            name: &str,
        ) -> ComponentId {
            let h = world.add_component_boxed_named(
                name,
                Box::new(TransformGizmoRotateComponent::new(axis)),
            );
            let _ = world.add_child(parent, h);
            h
        }

        // Axis colors.
        let red = [1.0, 0.15, 0.15, 1.0];
        let green = [0.15, 1.0, 0.15, 1.0];
        let blue = [0.15, 0.35, 1.0, 1.0];

        // Rotation rings (thin annulus) for X/Y/Z axes.
        let ring_mesh = CpuMeshHandle::CIRCLE_2D;
        let ring_scale = [1.4, 1.4, 1.0];

        // Rotation rings live under per-axis rotate handle components.
        let rot_x_root = spawn_rotate_handle_root(
            world,
            gizmo_visual_parent,
            TransformGizmoAxis::X,
            "gizmo_rot_x",
        );
        let rot_x_coord = spawn_gesture_coord_type_root(
            world,
            rot_x_root,
            "gizmo_rot_x_coord",
            GestureCoordType::WorldPlane,
        );
        let rot_x_pick = spawn_raycastable_root(world, rot_x_coord, "gizmo_rot_x_pick");
        spawn_part(
            world,
            rot_x_pick,
            "gizmo_rot_x_ring",
            ring_mesh,
            [0.0, 0.0, 0.0],
            [0.0, -std::f32::consts::FRAC_PI_2, 0.0],
            ring_scale,
            red,
        );

        let rot_y_root = spawn_rotate_handle_root(
            world,
            gizmo_visual_parent,
            TransformGizmoAxis::Y,
            "gizmo_rot_y",
        );
        let rot_y_coord = spawn_gesture_coord_type_root(
            world,
            rot_y_root,
            "gizmo_rot_y_coord",
            GestureCoordType::WorldPlane,
        );
        let rot_y_pick = spawn_raycastable_root(world, rot_y_coord, "gizmo_rot_y_pick");
        spawn_part(
            world,
            rot_y_pick,
            "gizmo_rot_y_ring",
            ring_mesh,
            [0.0, 0.0, 0.0],
            [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ring_scale,
            green,
        );

        let rot_z_root = spawn_rotate_handle_root(
            world,
            gizmo_visual_parent,
            TransformGizmoAxis::Z,
            "gizmo_rot_z",
        );
        let rot_z_coord = spawn_gesture_coord_type_root(
            world,
            rot_z_root,
            "gizmo_rot_z_coord",
            GestureCoordType::WorldPlane,
        );
        let rot_z_pick = spawn_raycastable_root(world, rot_z_coord, "gizmo_rot_z_pick");
        spawn_part(
            world,
            rot_z_pick,
            "gizmo_rot_z_ring",
            ring_mesh,
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            ring_scale,
            blue,
        );

        // Translation arrows: stem (cube) + cone tip.
        let stem_mesh = CpuMeshHandle::CUBE;
        let cone_mesh = CpuMeshHandle::CONE;
        let stem_len = 1.0_f32;
        let stem_thick = 0.06_f32;
        let cone_len = 0.22_f32;
        let cone_radius = 0.12_f32;

        // Translation arrows live under per-axis translate handle components.
        let move_x_root = spawn_translate_handle_root(
            world,
            gizmo_visual_parent,
            TransformGizmoAxis::X,
            "gizmo_move_x",
        );
        let move_x_pick = spawn_raycastable_root(world, move_x_root, "gizmo_move_x_pick");
        // +X arrow: rotate +Z axis to +X (yaw +90deg).
        let rot_x = [0.0, std::f32::consts::FRAC_PI_2, 0.0];
        spawn_part(
            world,
            move_x_pick,
            "gizmo_move_x_stem",
            stem_mesh,
            [stem_len * 0.5, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [stem_len, stem_thick, stem_thick],
            red,
        );
        spawn_part(
            world,
            move_x_pick,
            "gizmo_move_x_tip",
            cone_mesh,
            [stem_len + cone_len * 0.5, 0.0, 0.0],
            rot_x,
            [cone_radius, cone_radius, cone_len],
            red,
        );

        let move_y_root = spawn_translate_handle_root(
            world,
            gizmo_visual_parent,
            TransformGizmoAxis::Y,
            "gizmo_move_y",
        );
        let move_y_pick = spawn_raycastable_root(world, move_y_root, "gizmo_move_y_pick");
        // +Y arrow: rotate +Z axis to +Y (pitch -90deg around X).
        let rot_y = [-std::f32::consts::FRAC_PI_2, 0.0, 0.0];
        spawn_part(
            world,
            move_y_pick,
            "gizmo_move_y_stem",
            stem_mesh,
            [0.0, stem_len * 0.5, 0.0],
            [0.0, 0.0, 0.0],
            [stem_thick, stem_len, stem_thick],
            green,
        );
        spawn_part(
            world,
            move_y_pick,
            "gizmo_move_y_tip",
            cone_mesh,
            [0.0, stem_len + cone_len * 0.5, 0.0],
            rot_y,
            [cone_radius, cone_radius, cone_len],
            green,
        );

        let move_z_root = spawn_translate_handle_root(
            world,
            gizmo_visual_parent,
            TransformGizmoAxis::Z,
            "gizmo_move_z",
        );
        let move_z_pick = spawn_raycastable_root(world, move_z_root, "gizmo_move_z_pick");
        // +Z arrow: no rotation.
        spawn_part(
            world,
            move_z_pick,
            "gizmo_move_z_stem",
            stem_mesh,
            [0.0, 0.0, stem_len * 0.5],
            [0.0, 0.0, 0.0],
            [stem_thick, stem_thick, stem_len],
            blue,
        );
        spawn_part(
            world,
            move_z_pick,
            "gizmo_move_z_tip",
            cone_mesh,
            [0.0, 0.0, stem_len + cone_len * 0.5],
            [0.0, 0.0, 0.0],
            [cone_radius, cone_radius, cone_len],
            blue,
        );

        // Init the subtree (queues renderable/transform/color registrations).
        world.init_component_tree(gizmo_root, emit);
    }

    /// Resolve (gizmo, operation) for a hit renderable by walking up ancestry.
    ///
    /// Contract: the TRS handle component must be an ancestor of the clicked renderable.
    fn resolve_gizmo_op_for_renderable(
        world: &World,
        renderable: ComponentId,
    ) -> Option<(ComponentId, TransformGizmoOp)> {
        let mut cur = Some(renderable);
        let mut op: Option<TransformGizmoOp> = None;
        let mut gizmo: Option<ComponentId> = None;

        while let Some(node) = cur {
            if op.is_none() {
                if let Some(h) =
                    world.get_component_by_id_as::<TransformGizmoTranslateComponent>(node)
                {
                    op = Some(TransformGizmoOp::Translate(h.axis));
                } else if let Some(h) =
                    world.get_component_by_id_as::<TransformGizmoRotateComponent>(node)
                {
                    op = Some(TransformGizmoOp::Rotate(h.axis));
                } else if let Some(h) =
                    world.get_component_by_id_as::<TransformGizmoScaleComponent>(node)
                {
                    op = Some(TransformGizmoOp::Scale(h.axis));
                }
            }

            if gizmo.is_none()
                && world
                    .get_component_by_id_as::<TransformGizmoComponent>(node)
                    .is_some()
            {
                gizmo = Some(node);
            }

            if op.is_some() && gizmo.is_some() {
                break;
            }

            cur = world.parent_of(node);
        }

        Some((gizmo?, op?))
    }

    fn resolve_gesture_coord_type_for_renderable(
        world: &World,
        renderable: ComponentId,
    ) -> Option<GestureCoordType> {
        let mut cur = Some(renderable);
        while let Some(node) = cur {
            if let Some(c) = world.get_component_by_id_as::<GestureCoordTypeComponent>(node) {
                return Some(c.coord_type);
            }
            cur = world.parent_of(node);
        }
        None
    }

    fn gizmos_for_hit_renderable(world: &World, renderable: ComponentId) -> Vec<ComponentId> {
        let mut out: Vec<ComponentId> = world
            .children_of(renderable)
            .iter()
            .copied()
            .filter(|&ch| {
                world
                    .get_component_by_id_as::<TransformGizmoComponent>(ch)
                    .is_some()
            })
            .collect();

        // Also support gizmo-as-ancestor (new gizmo visuals are children of the gizmo node).
        let mut cur = Some(renderable);
        while let Some(node) = cur {
            if world
                .get_component_by_id_as::<TransformGizmoComponent>(node)
                .is_some()
            {
                out.push(node);
            }
            cur = world.parent_of(node);
        }

        out.sort();
        out.dedup();
        out
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        _input: &InputState,
        emit: &mut dyn SignalEmitter,
        _rx: &mut RxWorld,
    ) {
        // Handler-driven: drag + parent events are handled during drain points.
        // Keep `tick_with_queue` as a no-op entrypoint for now.
        let _ = (world, emit);
    }
}
