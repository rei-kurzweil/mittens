use crate::engine::ecs::component::{
    GizmoAxis, GizmoComponent, GizmoRotateComponent, GizmoScaleComponent, GizmoTranslateComponent,
    TransformComponent,
};
use crate::engine::ecs::{CommandQueue, ComponentId, EventSignal, RxWorld, SignalValue, World};
use crate::engine::user_input::InputState;

#[derive(Debug, Clone, Copy)]
enum GizmoOp {
    Translate(GizmoAxis),
    Rotate(GizmoAxis),
    Scale(GizmoAxis),
}

#[derive(Debug, Default)]
pub struct GizmoSystem;

impl GizmoSystem {
    pub fn new() -> Self {
        Self
    }

    /// Spawn the 9-part gizmo visual subtree for a GizmoComponent.
    ///
    /// Contract: GizmoComponent is expected to be attached under a TransformComponent.
    pub fn register_gizmo(
        &mut self,
        world: &mut World,
        component: ComponentId,
        queue: &mut CommandQueue,
    ) {
        use crate::engine::ecs::component::{
            GizmoAxis, GizmoComponent, GizmoRotateComponent, GizmoTranslateComponent,
            TransformComponent,
        };
        use crate::engine::graphics::primitives::CpuMeshHandle;

        // Must be a gizmo.
        let Some(_) = world.get_component_by_id_as::<GizmoComponent>(component) else {
            return;
        };

        // Find the nearest ancestor transform to attach visuals under.
        let mut cur = component;
        let mut parent_transform: Option<ComponentId> = None;
        while let Some(p) = world.parent_of(cur) {
            if world.get_component_by_id_as::<TransformComponent>(p).is_some() {
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
        if let Some(g) = world.get_component_by_id_as_mut::<GizmoComponent>(component) {
            g.target_transform = Some(parent_transform);
        }

        // Avoid respawn.
        if let Some(g) = world.get_component_by_id_as::<GizmoComponent>(component) {
            if g.visual_root.is_some() {
                return;
            }
        }

        // Create a root transform for the gizmo visuals under the GizmoComponent node.
        let gizmo_root = world.add_component_boxed_named(
            "gizmo_root",
            Box::new(TransformComponent::new()),
        );
        let _ = world.add_child(component, gizmo_root);

        // Write back visual root.
        if let Some(g) = world.get_component_by_id_as_mut::<GizmoComponent>(component) {
            g.visual_root = Some(gizmo_root);
        }

        // Helper: spawn a renderable under a transform with color+emissive+raycastable.
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
                ColorComponent, EmissiveComponent, RaycastableComponent, RenderableComponent,
                TransformComponent,
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
            let rc = world.add_component_boxed_named(
                format!("{name}_ray"),
                Box::new(RaycastableComponent::enabled()),
            );

            let _ = world.add_child(parent, t);
            let _ = world.add_child(t, r);
            let _ = world.add_child(r, c);
            let _ = world.add_child(r, e);
            let _ = world.add_child(r, rc);
        }

        fn spawn_translate_handle_root(
            world: &mut World,
            parent: ComponentId,
            axis: GizmoAxis,
            name: &str,
        ) -> ComponentId {
            let h = world.add_component_boxed_named(
                name,
                Box::new(GizmoTranslateComponent::new(axis)),
            );
            let _ = world.add_child(parent, h);
            h
        }

        fn spawn_rotate_handle_root(
            world: &mut World,
            parent: ComponentId,
            axis: GizmoAxis,
            name: &str,
        ) -> ComponentId {
            let h = world.add_component_boxed_named(name, Box::new(GizmoRotateComponent::new(axis)));
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
        let rot_x_root = spawn_rotate_handle_root(world, gizmo_root, GizmoAxis::X, "gizmo_rot_x");
        spawn_part(
            world,
            rot_x_root,
            "gizmo_rot_x_ring",
            ring_mesh,
            [0.0, 0.0, 0.0],
            [0.0, -std::f32::consts::FRAC_PI_2, 0.0],
            ring_scale,
            red,
        );

        let rot_y_root = spawn_rotate_handle_root(world, gizmo_root, GizmoAxis::Y, "gizmo_rot_y");
        spawn_part(
            world,
            rot_y_root,
            "gizmo_rot_y_ring",
            ring_mesh,
            [0.0, 0.0, 0.0],
            [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ring_scale,
            green,
        );

        let rot_z_root = spawn_rotate_handle_root(world, gizmo_root, GizmoAxis::Z, "gizmo_rot_z");
        spawn_part(
            world,
            rot_z_root,
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
        let move_x_root =
            spawn_translate_handle_root(world, gizmo_root, GizmoAxis::X, "gizmo_move_x");
        // +X arrow: rotate +Z axis to +X (yaw -90deg).
        let rot_x = [0.0, -std::f32::consts::FRAC_PI_2, 0.0];
        spawn_part(
            world,
            move_x_root,
            "gizmo_move_x_stem",
            stem_mesh,
            [stem_len * 0.5, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [stem_len, stem_thick, stem_thick],
            red,
        );
        spawn_part(
            world,
            move_x_root,
            "gizmo_move_x_tip",
            cone_mesh,
            [stem_len + cone_len * 0.5, 0.0, 0.0],
            rot_x,
            [cone_radius, cone_radius, cone_len],
            red,
        );

        let move_y_root =
            spawn_translate_handle_root(world, gizmo_root, GizmoAxis::Y, "gizmo_move_y");
        // +Y arrow: rotate +Z axis to +Y (pitch +90deg around X).
        let rot_y = [std::f32::consts::FRAC_PI_2, 0.0, 0.0];
        spawn_part(
            world,
            move_y_root,
            "gizmo_move_y_stem",
            stem_mesh,
            [0.0, stem_len * 0.5, 0.0],
            [0.0, 0.0, 0.0],
            [stem_thick, stem_len, stem_thick],
            green,
        );
        spawn_part(
            world,
            move_y_root,
            "gizmo_move_y_tip",
            cone_mesh,
            [0.0, stem_len + cone_len * 0.5, 0.0],
            rot_y,
            [cone_radius, cone_radius, cone_len],
            green,
        );

        let move_z_root =
            spawn_translate_handle_root(world, gizmo_root, GizmoAxis::Z, "gizmo_move_z");
        // +Z arrow: no rotation.
        spawn_part(
            world,
            move_z_root,
            "gizmo_move_z_stem",
            stem_mesh,
            [0.0, 0.0, stem_len * 0.5],
            [0.0, 0.0, 0.0],
            [stem_thick, stem_thick, stem_len],
            blue,
        );
        spawn_part(
            world,
            move_z_root,
            "gizmo_move_z_tip",
            cone_mesh,
            [0.0, 0.0, stem_len + cone_len * 0.5],
            [0.0, 0.0, 0.0],
            [cone_radius, cone_radius, cone_len],
            blue,
        );

        // Init the subtree (queues renderable/transform/color registrations).
        world.init_component_tree(gizmo_root, queue);
    }

    /// Resolve (gizmo, operation) for a hit renderable by walking up ancestry.
    ///
    /// Contract: the TRS handle component must be an ancestor of the clicked renderable.
    fn resolve_gizmo_op_for_renderable(
        world: &World,
        renderable: ComponentId,
    ) -> Option<(ComponentId, GizmoOp)> {
        let mut cur = Some(renderable);
        let mut op: Option<GizmoOp> = None;
        let mut gizmo: Option<ComponentId> = None;

        while let Some(node) = cur {
            if op.is_none() {
                if let Some(h) = world.get_component_by_id_as::<GizmoTranslateComponent>(node) {
                    op = Some(GizmoOp::Translate(h.axis));
                } else if let Some(h) = world.get_component_by_id_as::<GizmoRotateComponent>(node) {
                    op = Some(GizmoOp::Rotate(h.axis));
                } else if let Some(h) = world.get_component_by_id_as::<GizmoScaleComponent>(node) {
                    op = Some(GizmoOp::Scale(h.axis));
                }
            }

            if gizmo.is_none() && world.get_component_by_id_as::<GizmoComponent>(node).is_some() {
                gizmo = Some(node);
            }

            if op.is_some() && gizmo.is_some() {
                break;
            }

            cur = world.parent_of(node);
        }

        Some((gizmo?, op?))
    }

    fn gizmos_for_hit_renderable(world: &World, renderable: ComponentId) -> Vec<ComponentId> {
        let mut out: Vec<ComponentId> = world
            .children_of(renderable)
            .iter()
            .copied()
            .filter(|&ch| world.get_component_by_id_as::<GizmoComponent>(ch).is_some())
            .collect();

        // Also support gizmo-as-ancestor (new gizmo visuals are children of the gizmo node).
        let mut cur = Some(renderable);
        while let Some(node) = cur {
            if world.get_component_by_id_as::<GizmoComponent>(node).is_some() {
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
        queue: &mut crate::engine::ecs::CommandQueue,
        rx: &mut RxWorld,
    ) {
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

        // Snapshot drag events first (avoid borrowing issues while mutating the world).
        let mut drag_events: Vec<EventSignal> = Vec::new();
        for s in rx.signals().iter() {
            let SignalValue::Event(ev) = &s.value else {
                continue;
            };
            match ev {
                EventSignal::DragStart { .. }
                | EventSignal::DragMove { .. }
                | EventSignal::DragEnd { .. } => drag_events.push(ev.clone()),
                _ => {}
            }
        }

        for ev in drag_events {
            match ev {
                EventSignal::DragStart {
                    raycaster,
                    renderable,
                    ..
                } => {
                    let Some((gizmo_cid, _op)) =
                        Self::resolve_gizmo_op_for_renderable(world, renderable)
                    else {
                        continue;
                    };

                    if let Some(g) = world.get_component_by_id_as_mut::<GizmoComponent>(gizmo_cid) {
                        g.active_raycaster = Some(raycaster);
                    }
                }
                EventSignal::DragMove {
                    raycaster,
                    renderable,
                    delta_world,
                    hit_point,
                    ..
                } => {
                    let Some((gizmo_cid, op)) =
                        Self::resolve_gizmo_op_for_renderable(world, renderable)
                    else {
                        continue;
                    };

                    // Copy out what we need without holding a mutable borrow.
                    let Some((target_transform, active)) = world
                        .get_component_by_id_as::<GizmoComponent>(gizmo_cid)
                        .map(|g| (g.target_transform, g.active_raycaster))
                    else {
                        continue;
                    };

                    let Some(target_transform) = target_transform else {
                        continue;
                    };

                    if active != Some(raycaster) {
                        continue;
                    }

                    match op {
                        GizmoOp::Translate(axis) => {
                            let axis_v = axis.unit_vec3();
                            let d = dot(delta_world, axis_v);
                            let delta = mul(axis_v, d);

                            let Some(t) =
                                world.get_component_by_id_as_mut::<TransformComponent>(target_transform)
                            else {
                                continue;
                            };

                            let cur = t.transform.translation;
                            let next = add(cur, delta);
                            t.set_position(queue, next[0], next[1], next[2]);
                        }
                        GizmoOp::Rotate(axis) => {
                            let pivot = TransformSystem::world_position(world, target_transform)
                                .unwrap_or([0.0, 0.0, 0.0]);
                            let prev_hit = sub(hit_point, delta_world);

                            let axis_v = axis.unit_vec3();
                            let mut v0 = sub(prev_hit, pivot);
                            let mut v1 = sub(hit_point, pivot);

                            // Project onto plane orthogonal to the axis.
                            v0 = sub(v0, mul(axis_v, dot(v0, axis_v)));
                            v1 = sub(v1, mul(axis_v, dot(v1, axis_v)));
                            v0 = math::vec3_normalize(v0);
                            v1 = math::vec3_normalize(v1);

                            // Signed angle about axis.
                            let c = cross(v0, v1);
                            let s = dot(axis_v, c);
                            let d = dot(v0, v1);
                            let angle = s.atan2(d);

                            if angle != 0.0 {
                                let Some(t) = world
                                    .get_component_by_id_as_mut::<TransformComponent>(target_transform)
                                else {
                                    continue;
                                };
                                let q_delta = math::quat_from_axis_angle(axis_v, angle);
                                let q_next = math::quat_mul(q_delta, t.transform.rotation);
                                t.set_rotation_quat(queue, q_next);
                            }
                        }
                        GizmoOp::Scale(axis) => {
                            let d = dot(delta_world, axis.unit_vec3());

                            let Some(t) =
                                world.get_component_by_id_as_mut::<TransformComponent>(target_transform)
                            else {
                                continue;
                            };

                            let mut s = t.transform.scale;
                            match axis {
                                GizmoAxis::X => s[0] = (s[0] + d).max(0.001),
                                GizmoAxis::Y => s[1] = (s[1] + d).max(0.001),
                                GizmoAxis::Z => s[2] = (s[2] + d).max(0.001),
                            }
                            t.set_scale(queue, s[0], s[1], s[2]);
                        }
                    }
                }
                EventSignal::DragEnd {
                    raycaster,
                    renderable,
                    ..
                } => {
                    let Some((gizmo_cid, _op)) =
                        Self::resolve_gizmo_op_for_renderable(world, renderable)
                    else {
                        continue;
                    };

                    if let Some(g) = world.get_component_by_id_as_mut::<GizmoComponent>(gizmo_cid) {
                        if g.active_raycaster == Some(raycaster) {
                            g.active_raycaster = None;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
