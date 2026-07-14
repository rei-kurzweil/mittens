use crate::engine::ecs::component::{
    GestureCoordType, GestureCoordTypeComponent, SignalRouteUpwardComponent,
    TransformCameraSpecificComponent, TransformCameraSpecificMode, TransformComponent,
    TransformDropComponent, TransformForkTRSComponent, TransformGizmoAxis, TransformGizmoComponent,
    TransformGizmoRotateComponent, TransformGizmoScaleComponent, TransformGizmoTranslateComponent,
    TransformMapRotationComponent, TransformMapScaleComponent, TransformMapTranslationComponent,
};
use crate::engine::ecs::system::GridSystem;
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::engine::user_input::InputState;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy)]
enum TransformGizmoOp {
    Translate(TransformGizmoAxis),
    Rotate(TransformGizmoAxis),
    Scale(TransformGizmoAxis),
}

#[derive(Debug, Default)]
pub struct TransformGizmoSystem {
    editor_context_state: Option<Arc<Mutex<EditorContextState>>>,
    live_gizmos: std::collections::HashSet<ComponentId>,
}

impl TransformGizmoSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_editor_context_state(
        &mut self,
        editor_context_state: Arc<Mutex<EditorContextState>>,
    ) {
        self.editor_context_state = Some(editor_context_state);
    }

    /// Update the ordinary settings transforms used by camera-specific gizmo anchors.
    /// Returns anchors whose effective world matrices must be propagated immediately.
    pub fn update_camera_scales(
        &mut self,
        world: &mut World,
        visuals: &crate::engine::graphics::VisualWorld,
        has_window_camera: bool,
    ) -> Vec<ComponentId> {
        use crate::engine::graphics::CameraTarget;
        const REFERENCE_DISTANCE: f32 = 4.0;
        const MIN_SCALE: f32 = 0.02;
        const MAX_SCALE: f32 = 20.0;

        self.live_gizmos.retain(|id| {
            world
                .get_component_by_id_as::<TransformGizmoComponent>(*id)
                .is_some()
        });
        let stereo_eyes = visuals
            .visual_camera(CameraTarget::Xr)
            .filter(|c| visuals.active_xr_camera().is_some() && !c.eyes.is_empty());
        let stereo_active = stereo_eyes.is_some();
        let mut changed = Vec::new();

        for gizmo in self.live_gizmos.iter().copied().collect::<Vec<_>>() {
            let Some(g) = world
                .get_component_by_id_as::<TransformGizmoComponent>(gizmo)
                .copied()
            else {
                continue;
            };
            let Some(anchor) = g.visual_root else {
                continue;
            };
            let p = world
                .get_component_by_id_as::<TransformComponent>(anchor)
                .map(|t| t.transform.matrix_world[3])
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let views: Vec<[[f32; 4]; 4]> = if let Some(c) = stereo_eyes {
                c.eyes.iter().map(|e| e.view).collect()
            } else if has_window_camera {
                visuals
                    .visual_camera(CameraTarget::Window)
                    .and_then(|c| c.eyes.first())
                    .map(|e| vec![e.view])
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let depths: Vec<f32> = views
                .iter()
                .map(|v| {
                    let z = v[0][2] * p[0] + v[1][2] * p[1] + v[2][2] * p[2] + v[3][2];
                    -z
                })
                .collect();
            if depths.is_empty() || depths.iter().any(|d| !d.is_finite() || *d <= 0.0) {
                continue;
            }
            let depth = depths.iter().sum::<f32>() / depths.len() as f32;
            let scale = (g.scale * depth / REFERENCE_DISTANCE).clamp(MIN_SCALE, MAX_SCALE);

            let desired_mode = if stereo_active {
                TransformCameraSpecificMode::Stereoscopic
            } else {
                TransformCameraSpecificMode::Monoscopic
            };
            let settings = world
                .children_of(anchor)
                .iter()
                .copied()
                .find_map(|marker| {
                    let c =
                        world.get_component_by_id_as::<TransformCameraSpecificComponent>(marker)?;
                    (c.mode == desired_mode)
                        .then(|| {
                            world.children_of(marker).iter().copied().find(|id| {
                                world
                                    .get_component_by_id_as::<TransformComponent>(*id)
                                    .is_some()
                            })
                        })
                        .flatten()
                });
            if let Some(settings) = settings {
                if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(settings) {
                    t.transform.scale = [scale; 3];
                    t.transform.recompute_model();
                    changed.push(anchor);
                }
            }
        }
        changed
    }

    /// Install per-gizmo scoped handlers rooted at the `TransformGizmoComponent` node.
    ///
    /// Drag events are scoped to the hit renderable; because gizmo handle renderables live under
    /// the gizmo node, scoped handlers rooted at the gizmo will run for drag events on its handles.
    pub fn install_scoped_handlers_for_gizmo(&mut self, rx: &mut RxWorld, gizmo_root: ComponentId) {
        rx.add_handler(
            SignalKind::ParentChanged,
            gizmo_root,
            Self::on_parent_changed,
        );
        rx.add_handler(SignalKind::DragStart, gizmo_root, Self::on_drag_start);
        let editor_context_state = self.editor_context_state.clone();
        rx.add_handler_closure_named(
            SignalKind::DragMove,
            gizmo_root,
            None,
            move |world, emit, env| {
                Self::on_drag_move(world, emit, env, editor_context_state.clone());
            },
        );
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

    fn debug_target_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GIZMO_TARGET").unwrap_or_default();
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
    }

    fn debug_sanity_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GIZMO_SANITY").unwrap_or_default();
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
    }

    fn debug_apply_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GIZMO_APPLY").unwrap_or_default();
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
    }

    fn debug_hit_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_GIZMO_HIT").unwrap_or_default();
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
    }

    fn log_apply(world: &World, op: &str, target_transform: ComponentId, extra: &str) {
        static LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = LOG_COUNT.fetch_add(1, Ordering::Relaxed);
        if n >= 96 {
            return;
        }

        let name = world
            .get_component_record(target_transform)
            .map(|n| {
                if n.name.is_empty() {
                    n.component_type.clone()
                } else {
                    format!("{}: {}", n.component_type, n.name)
                }
            })
            .unwrap_or_else(|| "<missing>".to_string());

        println!(
            "[TransformGizmoSystem] APPLY op={} target={:?} '{}' {}",
            op, target_transform, name, extra
        );
    }

    fn sanity_check_transform_values(
        world: &World,
        target_transform: ComponentId,
        translation: [f32; 3],
        rotation_xyzw: [f32; 4],
        scale: [f32; 3],
    ) {
        fn finite_f32(x: f32) -> bool {
            x.is_finite()
        }
        fn finite3(v: [f32; 3]) -> bool {
            finite_f32(v[0]) && finite_f32(v[1]) && finite_f32(v[2])
        }
        fn finite4(v: [f32; 4]) -> bool {
            finite_f32(v[0]) && finite_f32(v[1]) && finite_f32(v[2]) && finite_f32(v[3])
        }
        fn too_large3(v: [f32; 3]) -> bool {
            let lim = 1.0e6_f32;
            v[0].abs() > lim || v[1].abs() > lim || v[2].abs() > lim
        }

        if finite3(translation)
            && finite4(rotation_xyzw)
            && finite3(scale)
            && !too_large3(translation)
            && !too_large3(scale)
        {
            return;
        }

        static LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = LOG_COUNT.fetch_add(1, Ordering::Relaxed);
        if n >= 32 {
            return;
        }

        let name = world
            .get_component_record(target_transform)
            .map(|n| {
                if n.name.is_empty() {
                    n.component_type.clone()
                } else {
                    format!("{}: {}", n.component_type, n.name)
                }
            })
            .unwrap_or_else(|| "<missing>".to_string());

        println!(
            "[TransformGizmoSystem] SANITY target={:?} '{}' translation={:?} rotation={:?} scale={:?}",
            target_transform, name, translation, rotation_xyzw, scale
        );
    }

    fn apply_route_upward_if_present(
        world: &World,
        kind_name: &str,
        start: ComponentId,
    ) -> ComponentId {
        let mut cur_target = start;

        // Apply all child route-up operators in order of appearance.
        // (In current usage there will typically be 0 or 1.)
        for &ch in world.children_of(start) {
            let Some(op) = world.get_component_by_id_as::<SignalRouteUpwardComponent>(ch) else {
                continue;
            };

            let want = op.intent_kind.trim();
            let applies = want.is_empty() || want == "any" || want == kind_name;
            if !applies {
                continue;
            }

            let parent_type = op.parent_type.trim();
            if parent_type.is_empty() {
                continue;
            }

            // Ancestor search: do not match the start node itself.
            let mut cur = world.parent_of(cur_target);
            while let Some(cid) = cur {
                let Some(node) = world.get_component_node(cid) else {
                    break;
                };

                if node.component.name() == parent_type {
                    cur_target = cid;
                    break;
                }

                cur = world.parent_of(cid);
            }
        }

        cur_target
    }

    fn mat4_identity() -> crate::engine::graphics::primitives::TransformMatrix {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn mat4_mul_vec4(
        m: crate::engine::graphics::primitives::TransformMatrix,
        v: [f32; 4],
    ) -> [f32; 4] {
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
            m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
        ]
    }

    fn parent_transform_world_matrix(
        world: &World,
        transform_cid: ComponentId,
    ) -> Option<crate::engine::graphics::primitives::TransformMatrix> {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                return Some(t.transform.matrix_world);
            }
            cur = parent;
        }
        None
    }

    fn world_delta_to_target_local(
        world: &World,
        target_transform: ComponentId,
        delta_world: [f32; 3],
    ) -> [f32; 3] {
        use crate::utils::math;

        let parent_world = Self::parent_transform_world_matrix(world, target_transform)
            .unwrap_or_else(Self::mat4_identity);
        let inv_parent_world = math::mat4_inverse(parent_world).unwrap_or_else(Self::mat4_identity);

        let v = Self::mat4_mul_vec4(
            inv_parent_world,
            [delta_world[0], delta_world[1], delta_world[2], 0.0],
        );
        [v[0], v[1], v[2]]
    }

    fn target_translation_local_to_world(
        world: &World,
        target_transform: ComponentId,
        point_local: [f32; 3],
    ) -> [f32; 3] {
        let parent_world = Self::parent_transform_world_matrix(world, target_transform)
            .unwrap_or_else(Self::mat4_identity);
        let v = Self::mat4_mul_vec4(
            parent_world,
            [point_local[0], point_local[1], point_local[2], 1.0],
        );
        [v[0], v[1], v[2]]
    }

    fn world_point_to_target_translation_local(
        world: &World,
        target_transform: ComponentId,
        point_world: [f32; 3],
    ) -> [f32; 3] {
        use crate::utils::math;

        let parent_world = Self::parent_transform_world_matrix(world, target_transform)
            .unwrap_or_else(Self::mat4_identity);
        let inv_parent_world = math::mat4_inverse(parent_world).unwrap_or_else(Self::mat4_identity);
        let v = Self::mat4_mul_vec4(
            inv_parent_world,
            [point_world[0], point_world[1], point_world[2], 1.0],
        );
        [v[0], v[1], v[2]]
    }

    fn active_snap_grid_for_translate(
        world: &World,
        editor_context_state: Option<Arc<Mutex<EditorContextState>>>,
    ) -> Option<crate::engine::ecs::system::grid_system::ActiveGrid> {
        let owner_transform = editor_context_state
            .as_ref()
            .and_then(|state| state.lock().ok())
            .and_then(|state| state.active_grid_owner_transform)?;
        let grids = GridSystem::new();
        grids.active_grid_for_owner_transform(world, owner_transform)
    }

    fn world_dir_to_target_local(
        world: &World,
        target_transform: ComponentId,
        dir_world: [f32; 3],
    ) -> [f32; 3] {
        use crate::utils::math;

        let d = Self::world_delta_to_target_local(world, target_transform, dir_world);
        math::vec3_normalize(d)
    }

    fn translation_drag_next_local(
        world: &World,
        target_transform: ComponentId,
        axis_world: [f32; 3],
        drag_start_hit_point_world: [f32; 3],
        drag_start_target_translation: [f32; 3],
        current_hit_point_world: [f32; 3],
    ) -> [f32; 3] {
        fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
        }

        fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
        }

        fn mul(v: [f32; 3], s: f32) -> [f32; 3] {
            [v[0] * s, v[1] * s, v[2] * s]
        }

        fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
        }

        let drag_world = sub(current_hit_point_world, drag_start_hit_point_world);
        let axis_distance_world = dot(drag_world, axis_world);
        let delta_world_axis = mul(axis_world, axis_distance_world);
        let delta_local =
            Self::world_delta_to_target_local(world, target_transform, delta_world_axis);
        add(drag_start_target_translation, delta_local)
    }

    fn resolve_translation_space(
        world: &World,
        gizmo_cid: ComponentId,
    ) -> crate::engine::ecs::component::TransformGizmoCoordSpace {
        let mut translation_space = crate::engine::ecs::component::TransformGizmoCoordSpace::World;
        let mut cur = Some(gizmo_cid);
        while let Some(node) = cur {
            if let Some(ed) =
                world.get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(node)
            {
                translation_space = ed.transform_gizmo_translation_space;
                break;
            }
            cur = world.parent_of(node);
        }
        translation_space
    }

    fn transform_direction(
        m: crate::engine::graphics::primitives::TransformMatrix,
        v: [f32; 3],
    ) -> [f32; 3] {
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
        ]
    }

    fn translation_axis_world(
        world: &World,
        target_transform: ComponentId,
        space: crate::engine::ecs::component::TransformGizmoCoordSpace,
        axis: TransformGizmoAxis,
    ) -> [f32; 3] {
        use crate::engine::ecs::system::transform_system::TransformSystem;
        use crate::utils::math;

        let axis_local = axis.unit_vec3();
        match space {
            crate::engine::ecs::component::TransformGizmoCoordSpace::World => axis_local,
            crate::engine::ecs::component::TransformGizmoCoordSpace::Local => {
                let target_world = TransformSystem::world_model(world, target_transform)
                    .unwrap_or_else(Self::mat4_identity);
                math::vec3_normalize(Self::transform_direction(target_world, axis_local))
            }
        }
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

    fn on_parent_changed(
        world: &mut World,
        _emit: &mut dyn SignalEmitter,
        env: &crate::engine::ecs::Signal,
    ) {
        let Some(EventSignal::ParentChanged {
            child, new_parent, ..
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
            if world
                .get_component_by_id_as::<TransformComponent>(node)
                .is_some()
            {
                target = Some(node);
                break;
            }
            cur = world.parent_of(node);
        }

        let old_target = world
            .get_component_by_id_as::<TransformGizmoComponent>(*child)
            .and_then(|g| g.target_transform);

        // If the newly-selected transform is a proxy (e.g. glTF viz:* transform), allow it to
        // carry routing operators that redirect gizmo edits to an ancestor target.
        let routed_target =
            target.map(|t| Self::apply_route_upward_if_present(world, "update_transform", t));

        if Self::debug_target_enabled() {
            if let (Some(orig), Some(routed)) = (target, routed_target) {
                if orig != routed {
                    let orig_name = world
                        .get_component_record(orig)
                        .map(|n| {
                            if n.name.is_empty() {
                                n.component_type.clone()
                            } else {
                                format!("{}: {}", n.component_type, n.name)
                            }
                        })
                        .unwrap_or_else(|| "<missing>".to_string());
                    let routed_name = world
                        .get_component_record(routed)
                        .map(|n| {
                            if n.name.is_empty() {
                                n.component_type.clone()
                            } else {
                                format!("{}: {}", n.component_type, n.name)
                            }
                        })
                        .unwrap_or_else(|| "<missing>".to_string());
                    println!(
                        "[TransformGizmoSystem] routed target_transform {:?} '{}' -> {:?} '{}'",
                        orig, orig_name, routed, routed_name
                    );
                }
            }
        }

        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(*child) {
            g.target_transform = routed_target;
            g.active_raycaster = None;
        }

        if Self::debug_enabled() {
            println!(
                "[TransformGizmoSystem] ParentChanged gizmo={:?} new_parent={:?} old_target={:?} new_target={:?}",
                child, new_parent, old_target, target
            );
        }
    }

    fn on_drag_start(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        env: &crate::engine::ecs::Signal,
    ) {
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

        let Some((gizmo_cid, _op)) = Self::resolve_gizmo_op_for_renderable(world, *renderable)
        else {
            return;
        };

        let drag_start_target_translation = world
            .get_component_by_id_as::<TransformGizmoComponent>(gizmo_cid)
            .and_then(|g| g.target_transform)
            .and_then(|target| world.get_component_by_id_as::<TransformComponent>(target))
            .map(|t| t.transform.translation);

        let mut old_debug_root: Option<ComponentId> = None;
        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid) {
            g.active_raycaster = Some(*raycaster);
            g.active_drag_slider_last_angle = 0.0;
            g.active_drag_start_hit_point_world = Some(*hit_point);
            g.active_drag_start_target_translation = drag_start_target_translation;
            if Self::debug_drag_plane_enabled() {
                old_debug_root = g.debug_drag_plane_root.take();
            }
        }

        if let Some(root) = old_debug_root {
            emit.push_intent_now(
                root,
                IntentValue::RemoveSubtree {
                    component_ids: vec![root],
                },
            );
        }

        if Self::debug_drag_plane_enabled() {
            let plane_root = Self::spawn_debug_drag_plane(world, emit, *hit_point, *ray_dir_world);
            if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid)
            {
                g.debug_drag_plane_root = Some(plane_root);
            }
        }
    }

    fn on_drag_move(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        env: &crate::engine::ecs::Signal,
        editor_context_state: Option<Arc<Mutex<EditorContextState>>>,
    ) {
        use crate::engine::ecs::system::transform_system::TransformSystem;
        use crate::utils::math;

        fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
        }

        fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
            [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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
            screen_pos_px: _screen_pos_px,
            screen_delta_px,
            ..
        }) = env.event.as_ref()
        else {
            return;
        };

        let Some((gizmo_cid, op)) = Self::resolve_gizmo_op_for_renderable(world, *renderable)
        else {
            return;
        };

        // Copy out what we need without holding a mutable borrow.
        let Some((
            target_transform,
            active,
            slider_last_angle,
            drag_start_hit_point_world,
            drag_start_target_translation,
        )) = world
            .get_component_by_id_as::<TransformGizmoComponent>(gizmo_cid)
            .map(|g| {
                (
                    g.target_transform,
                    g.active_raycaster,
                    g.active_drag_slider_last_angle,
                    g.active_drag_start_hit_point_world,
                    g.active_drag_start_target_translation,
                )
            })
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
                let translation_space = Self::resolve_translation_space(world, gizmo_cid);
                let axis_v =
                    Self::translation_axis_world(world, target_transform, translation_space, axis);
                let Some(drag_start_hit_point_world) = drag_start_hit_point_world else {
                    return;
                };
                let Some(drag_start_target_translation) = drag_start_target_translation else {
                    return;
                };
                let unsnapped_next = Self::translation_drag_next_local(
                    world,
                    target_transform,
                    axis_v,
                    drag_start_hit_point_world,
                    drag_start_target_translation,
                    *hit_point,
                );
                let next = if let Some(active_grid) =
                    Self::active_snap_grid_for_translate(world, editor_context_state.clone())
                {
                    let candidate_world = Self::target_translation_local_to_world(
                        world,
                        target_transform,
                        unsnapped_next,
                    );
                    let snapped_world = GridSystem::snap_point_preserving_plane_offset(
                        &active_grid,
                        candidate_world,
                    )
                    .point_world;
                    Self::world_point_to_target_translation_local(
                        world,
                        target_transform,
                        snapped_world,
                    )
                } else {
                    unsnapped_next
                };

                if Self::debug_apply_enabled() {
                    Self::log_apply(
                        world,
                        "translate",
                        target_transform,
                        &format!(
                            "translation_space={:?} hit_point={:?} drag_start_hit_point={:?} axis_world={:?} drag_start_t={:?} next_t={:?}",
                            translation_space,
                            *hit_point,
                            drag_start_hit_point_world,
                            axis_v,
                            drag_start_target_translation,
                            next,
                        ),
                    );
                }

                let Some(t_ro) =
                    world.get_component_by_id_as::<TransformComponent>(target_transform)
                else {
                    return;
                };
                if Self::debug_sanity_enabled() {
                    Self::sanity_check_transform_values(
                        world,
                        target_transform,
                        next,
                        t_ro.transform.rotation,
                        t_ro.transform.scale,
                    );
                }

                let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(target_transform)
                else {
                    return;
                };
                t.set_position(emit, next[0], next[1], next[2]);
            }
            TransformGizmoOp::Rotate(axis) => {
                let coord_type =
                    Self::resolve_gesture_coord_type_for_renderable(world, *renderable);

                // Resolve rotation coord space (default Local). This controls how we interpret the
                // axis when applying the drag angle.
                let mut rotation_space =
                    crate::engine::ecs::component::TransformGizmoCoordSpace::Local;
                {
                    let mut cur = Some(gizmo_cid);
                    while let Some(node) = cur {
                        if let Some(ed) = world.get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(node) {
                            rotation_space = ed.transform_gizmo_rotation_space;
                            break;
                        }
                        cur = world.parent_of(node);
                    }
                }

                let axis_v = axis.unit_vec3();
                let (angle, new_slider_last_angle) = match coord_type {
                    Some(GestureCoordType::ScreenSpace1DSlider) => {
                        match *screen_delta_px {
                            Some((dx, dy)) => {
                                // Incremental mapping: avoid any “flip” behavior caused by crossing
                                // a reference vector/origin by only integrating the per-move delta.
                                let radians_per_px = 0.01_f32;
                                let delta_px = dx + dy;
                                let delta_angle = delta_px * radians_per_px;
                                (delta_angle, slider_last_angle + delta_angle)
                            }
                            None => {
                                // If screen deltas aren't available (e.g. XR pointers), do nothing.
                                (0.0_f32, slider_last_angle)
                            }
                        }
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
                        (s.atan2(d), slider_last_angle)
                    }
                };

                if angle != 0.0 {
                    let axis_local = match rotation_space {
                        crate::engine::ecs::component::TransformGizmoCoordSpace::Local => axis_v,
                        // World mode will be implemented next; for now keep the previous behavior
                        // (convert the world axis into target-local space).
                        crate::engine::ecs::component::TransformGizmoCoordSpace::World => {
                            Self::world_dir_to_target_local(world, target_transform, axis_v)
                        }
                    };

                    let Some(t_ro) =
                        world.get_component_by_id_as::<TransformComponent>(target_transform)
                    else {
                        return;
                    };
                    let q_delta_local = math::quat_from_axis_angle(axis_local, angle);
                    // Quaternion multiplication order determines the frame the delta is applied in:
                    // - Local: post-multiply (rotate in the object's local frame)
                    // - World: pre-multiply (rotate in the parent/world frame)
                    let q_next = match rotation_space {
                        crate::engine::ecs::component::TransformGizmoCoordSpace::Local => {
                            math::quat_mul(t_ro.transform.rotation, q_delta_local)
                        }
                        crate::engine::ecs::component::TransformGizmoCoordSpace::World => {
                            math::quat_mul(q_delta_local, t_ro.transform.rotation)
                        }
                    };

                    if Self::debug_apply_enabled() {
                        Self::log_apply(
                            world,
                            "rotate",
                            target_transform,
                            &format!(
                                "delta_world={:?} axis_world={:?} axis_local={:?} angle={:.6} cur_q={:?} next_q={:?} pivot_world={:?}",
                                *delta_world,
                                axis_v,
                                axis_local,
                                angle,
                                t_ro.transform.rotation,
                                q_next,
                                TransformSystem::world_position(world, target_transform)
                                    .unwrap_or([0.0, 0.0, 0.0]),
                            ),
                        );
                    }

                    if Self::debug_sanity_enabled() {
                        Self::sanity_check_transform_values(
                            world,
                            target_transform,
                            t_ro.transform.translation,
                            q_next,
                            t_ro.transform.scale,
                        );
                    }

                    let Some(t) =
                        world.get_component_by_id_as_mut::<TransformComponent>(target_transform)
                    else {
                        return;
                    };
                    t.set_rotation_quat(emit, q_next);
                }

                if coord_type == Some(GestureCoordType::ScreenSpace1DSlider) {
                    if let Some(g) =
                        world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid)
                    {
                        g.active_drag_slider_last_angle = new_slider_last_angle;
                    }
                }
            }
            TransformGizmoOp::Scale(axis) => {
                let d = dot(*delta_world, axis.unit_vec3());

                // Convert the world-space drag delta into target-local space so scaling behaves
                // consistently even when the target has a rotated/scaled parent.
                let delta_world_axis = mul(axis.unit_vec3(), d);
                let delta_local_axis =
                    Self::world_delta_to_target_local(world, target_transform, delta_world_axis);
                let axis_local_dir =
                    Self::world_dir_to_target_local(world, target_transform, axis.unit_vec3());
                let d_local = dot(delta_local_axis, axis_local_dir);

                let Some(t_ro) =
                    world.get_component_by_id_as::<TransformComponent>(target_transform)
                else {
                    return;
                };
                let mut s = t_ro.transform.scale;
                match axis {
                    TransformGizmoAxis::X => s[0] = (s[0] + d_local).max(0.001),
                    TransformGizmoAxis::Y => s[1] = (s[1] + d_local).max(0.001),
                    TransformGizmoAxis::Z => s[2] = (s[2] + d_local).max(0.001),
                }

                if Self::debug_apply_enabled() {
                    Self::log_apply(
                        world,
                        "scale",
                        target_transform,
                        &format!(
                            "delta_world={:?} axis_world={:?} d_world={:.6} delta_world_axis={:?} delta_local_axis={:?} axis_local_dir={:?} d_local={:.6} cur_s={:?} next_s={:?}",
                            *delta_world,
                            axis.unit_vec3(),
                            d,
                            delta_world_axis,
                            delta_local_axis,
                            axis_local_dir,
                            d_local,
                            t_ro.transform.scale,
                            s,
                        ),
                    );
                }

                if Self::debug_sanity_enabled() {
                    Self::sanity_check_transform_values(
                        world,
                        target_transform,
                        t_ro.transform.translation,
                        t_ro.transform.rotation,
                        s,
                    );
                }

                let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(target_transform)
                else {
                    return;
                };
                t.set_scale(emit, s[0], s[1], s[2]);
            }
        }
    }

    fn on_drag_end(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        env: &crate::engine::ecs::Signal,
    ) {
        let Some(EventSignal::DragEnd {
            raycaster,
            renderable,
            ..
        }) = env.event.as_ref()
        else {
            return;
        };

        let Some((gizmo_cid, _op)) = Self::resolve_gizmo_op_for_renderable(world, *renderable)
        else {
            return;
        };

        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(gizmo_cid) {
            if g.active_raycaster == Some(*raycaster) {
                g.active_raycaster = None;
            }
            g.active_drag_slider_last_angle = 0.0;
            g.active_drag_start_hit_point_world = None;
            g.active_drag_start_target_translation = None;

            if Self::debug_drag_plane_enabled() {
                if let Some(root) = g.debug_drag_plane_root.take() {
                    emit.push_intent_now(
                        root,
                        IntentValue::RemoveSubtree {
                            component_ids: vec![root],
                        },
                    );
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
            EditorComponent, OverlayComponent, TransformComponent, TransformGizmoAxis,
            TransformGizmoComponent, TransformGizmoCoordSpace, TransformGizmoRotateComponent,
            TransformGizmoTranslateComponent,
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
        // scale the target (and its ancestors) have.
        //
        // We now spawn gizmo visuals under an explicit transform pipeline that keeps translation
        // + rotation but drops inherited scale. This prevents non-uniform target scales from
        // squashing the gizmo and means `TransformGizmoComponent.scale` can be interpreted
        // directly as a world-ish size knob (modulo camera projection).
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
        let gizmo_local_scale = gizmo_scale;

        fn add_pipeline_group(
            world: &mut World,
            parent: ComponentId,
            pipeline_name: &str,
            include_translation_map: bool,
            include_rotation_map: bool,
            include_scale_map: bool,
            drop_translation: bool,
            drop_rotation: bool,
            drop_scale: bool,
        ) -> ComponentId {
            let fork = world.add_component_boxed_named(
                pipeline_name,
                Box::new(TransformForkTRSComponent::new()),
            );
            let _ = world.add_child(parent, fork);

            if include_translation_map {
                let map = world.add_component_boxed_named(
                    format!("{pipeline_name}:map_translation"),
                    Box::new(TransformMapTranslationComponent::new()),
                );
                let _ = world.add_child(fork, map);
                if drop_translation {
                    let drop = world.add_component_boxed_named(
                        format!("{pipeline_name}:drop_translation"),
                        Box::new(TransformDropComponent::new()),
                    );
                    let _ = world.add_child(map, drop);
                }
            }

            if include_rotation_map {
                let map = world.add_component_boxed_named(
                    format!("{pipeline_name}:map_rotation"),
                    Box::new(TransformMapRotationComponent::new()),
                );
                let _ = world.add_child(fork, map);
                if drop_rotation {
                    let drop = world.add_component_boxed_named(
                        format!("{pipeline_name}:drop_rotation"),
                        Box::new(TransformDropComponent::new()),
                    );
                    let _ = world.add_child(map, drop);
                }
            }

            if include_scale_map {
                let map = world.add_component_boxed_named(
                    format!("{pipeline_name}:map_scale"),
                    Box::new(TransformMapScaleComponent::new()),
                );
                let _ = world.add_child(fork, map);
                if drop_scale {
                    let drop = world.add_component_boxed_named(
                        format!("{pipeline_name}:drop_scale"),
                        Box::new(TransformDropComponent::new()),
                    );
                    let _ = world.add_child(map, drop);
                }
            }

            fork
        }

        if Self::debug_enabled() {
            println!(
                "[TransformGizmoSystem] register gizmo={:?} target_transform={:?} requested_world_scale={:.4} parent_world_scale={:.4} gizmo_local_scale={:.4}",
                component, parent_transform, gizmo_scale, parent_world_scale, gizmo_local_scale
            );
        }

        // Process inherited transforms from the target explicitly via a transform pipeline:
        // keep translation + rotation, drop scale.
        let gizmo_output = add_pipeline_group(
            world,
            component,
            "gizmo_pipeline",
            true,
            true,
            true,
            false,
            false,
            true,
        );

        // Create a root transform for the gizmo visuals under the GizmoComponent node.
        let gizmo_root =
            world.add_component_boxed_named("gizmo_root", Box::new(TransformComponent::new()));
        let _ = world.add_child(gizmo_output, gizmo_root);

        // Camera-specific configuration children are ordinary TRS values. The stream
        // evaluates the anchor first and then composes exactly one settings transform.
        for (name, marker) in [
            (
                "gizmo_camera_mono",
                TransformCameraSpecificComponent::active_monoscopic(),
            ),
            (
                "gizmo_camera_stereo",
                TransformCameraSpecificComponent::active_stereoscopic(),
            ),
        ] {
            let mode = world.add_component_boxed_named(name, Box::new(marker));
            let settings = world.add_component_boxed_named(
                format!("{name}_settings"),
                Box::new(TransformComponent::new().with_scale(
                    gizmo_local_scale,
                    gizmo_local_scale,
                    gizmo_local_scale,
                )),
            );
            let _ = world.add_child(gizmo_root, mode);
            let _ = world.add_child(mode, settings);
        }

        // Wrap all gizmo visuals in an overlay marker so they render in the overlay pass.
        let gizmo_overlay =
            world.add_component_boxed_named("gizmo_overlay", Box::new(OverlayComponent::new()));
        let _ = world.add_child(gizmo_root, gizmo_overlay);

        let gizmo_visual_parent = gizmo_overlay;

        // Resolve editor settings (coord spaces) by walking up ancestry to the nearest EditorComponent.
        let mut translation_space = TransformGizmoCoordSpace::World;
        let mut rotation_space = TransformGizmoCoordSpace::Local;
        {
            let mut cur = Some(component);
            while let Some(node) = cur {
                if let Some(ed) = world.get_component_by_id_as::<EditorComponent>(node) {
                    translation_space = ed.transform_gizmo_translation_space;
                    rotation_space = ed.transform_gizmo_rotation_space;
                    break;
                }
                cur = world.parent_of(node);
            }
        }

        // Create two coord-space groups so translation and rotation handles can be oriented
        // independently (e.g. translate in World while rotating in Local).
        //
        // These pipeline groups sit under the gizmo's uniform scale transform (`gizmo_root`), so
        // they keep gizmo size but can optionally drop inherited rotation.
        let gizmo_space_world = add_pipeline_group(
            world,
            gizmo_visual_parent,
            "gizmo_space_world_pipeline",
            true,
            true,
            true,
            false,
            true,
            false,
        );
        let gizmo_space_local = add_pipeline_group(
            world,
            gizmo_visual_parent,
            "gizmo_space_local_pipeline",
            true,
            true,
            true,
            false,
            false,
            false,
        );

        let translate_parent = match translation_space {
            TransformGizmoCoordSpace::World => gizmo_space_world,
            TransformGizmoCoordSpace::Local => gizmo_space_local,
        };

        let rotate_parent = match rotation_space {
            TransformGizmoCoordSpace::World => gizmo_space_world,
            TransformGizmoCoordSpace::Local => gizmo_space_local,
        };

        // Write back visual root.
        if let Some(g) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(component) {
            g.visual_root = Some(gizmo_root);
        }
        self.live_gizmos.insert(component);

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

            let rc = world.add_component_boxed_named(
                name,
                Box::new(RaycastableComponent::drag_only().with_interaction_priority(1)),
            );
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
        let rot_x_root =
            spawn_rotate_handle_root(world, rotate_parent, TransformGizmoAxis::X, "gizmo_rot_x");
        let rot_x_coord = spawn_gesture_coord_type_root(
            world,
            rot_x_root,
            "gizmo_rot_x_coord",
            GestureCoordType::ScreenSpace1DSlider,
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

        let rot_y_root =
            spawn_rotate_handle_root(world, rotate_parent, TransformGizmoAxis::Y, "gizmo_rot_y");
        let rot_y_coord = spawn_gesture_coord_type_root(
            world,
            rot_y_root,
            "gizmo_rot_y_coord",
            GestureCoordType::ScreenSpace1DSlider,
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

        let rot_z_root =
            spawn_rotate_handle_root(world, rotate_parent, TransformGizmoAxis::Z, "gizmo_rot_z");
        let rot_z_coord = spawn_gesture_coord_type_root(
            world,
            rot_z_root,
            "gizmo_rot_z_coord",
            GestureCoordType::ScreenSpace1DSlider,
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
            translate_parent,
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
            translate_parent,
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
            translate_parent,
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

        let resolved = Some((gizmo?, op?));

        if Self::debug_hit_enabled() {
            let (gizmo_cid, op) = resolved?;
            let renderable_name = world
                .get_component_record(renderable)
                .map(|n| {
                    if n.name.is_empty() {
                        n.component_type.clone()
                    } else {
                        format!("{}: {}", n.component_type, n.name)
                    }
                })
                .unwrap_or_else(|| "<missing>".to_string());
            let gizmo_name = world
                .get_component_record(gizmo_cid)
                .map(|n| {
                    if n.name.is_empty() {
                        n.component_type.clone()
                    } else {
                        format!("{}: {}", n.component_type, n.name)
                    }
                })
                .unwrap_or_else(|| "<missing>".to_string());
            let target_transform = world
                .get_component_by_id_as::<TransformGizmoComponent>(gizmo_cid)
                .and_then(|g| g.target_transform);
            let target_name = target_transform
                .and_then(|cid| world.get_component_record(cid).map(|n| (cid, n)))
                .map(|(cid, n)| {
                    if n.name.is_empty() {
                        format!("{cid:?} '{}'", n.component_type)
                    } else {
                        format!("{cid:?} '{}: {}'", n.component_type, n.name)
                    }
                })
                .unwrap_or_else(|| "<none>".to_string());
            println!(
                "[TransformGizmoSystem] resolve_gizmo_op renderable={renderable:?} '{}' gizmo={gizmo_cid:?} '{}' op={op:?} target={target_name}",
                renderable_name, gizmo_name,
            );
            return Some((gizmo_cid, op));
        }

        resolved
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

    #[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::TransformGizmoSystem;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        TransformComponent, TransformGizmoAxis, TransformGizmoCoordSpace,
    };

    fn approx3(a: [f32; 3], b: [f32; 3]) {
        for i in 0..3 {
            assert!(
                (a[i] - b[i]).abs() < 1e-4,
                "index {i}: left={:?} right={:?}",
                a,
                b
            );
        }
    }

    #[test]
    fn world_delta_is_converted_through_rotated_parent_inverse() {
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new().with_rotation_euler(
            0.0,
            0.0,
            std::f32::consts::FRAC_PI_2,
        ));
        let target = world.add_component(TransformComponent::new());
        world.add_child(parent, target).expect("attach target");

        let parent_world = world
            .get_component_by_id_as::<TransformComponent>(parent)
            .expect("parent transform")
            .transform
            .model;
        world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .expect("parent transform")
            .transform
            .matrix_world = parent_world;

        let local =
            TransformGizmoSystem::world_delta_to_target_local(&world, target, [1.0, 0.0, 0.0]);
        approx3(local, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn local_translation_axis_uses_target_world_rotation() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new().with_rotation_euler(
            0.0,
            0.0,
            std::f32::consts::FRAC_PI_2,
        ));
        let target_world = world
            .get_component_by_id_as::<TransformComponent>(target)
            .expect("target transform")
            .transform
            .model;
        world
            .get_component_by_id_as_mut::<TransformComponent>(target)
            .expect("target transform")
            .transform
            .matrix_world = target_world;
        let axis = TransformGizmoSystem::translation_axis_world(
            &world,
            target,
            TransformGizmoCoordSpace::Local,
            TransformGizmoAxis::X,
        );
        approx3(axis, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn translation_drag_uses_drag_start_anchor_instead_of_frame_delta() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new().with_position(10.0, 0.0, 0.0));

        let next = TransformGizmoSystem::translation_drag_next_local(
            &world,
            target,
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.25, 0.5, 0.0],
        );

        approx3(next, [10.25, 0.0, 0.0]);
    }

    #[test]
    fn translation_drag_respects_rotated_parent_space_from_drag_start() {
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new().with_rotation_euler(
            0.0,
            0.0,
            std::f32::consts::FRAC_PI_2,
        ));
        let target = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
        world.add_child(parent, target).expect("attach target");

        let parent_world = world
            .get_component_by_id_as::<TransformComponent>(parent)
            .expect("parent transform")
            .transform
            .model;
        world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .expect("parent transform")
            .transform
            .matrix_world = parent_world;

        let next = TransformGizmoSystem::translation_drag_next_local(
            &world,
            target,
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [1.0, 0.25, 0.0],
        );

        approx3(next, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn translation_local_world_conversion_uses_parent_space_not_target_rotation() {
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new());
        let target = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 2.0, 3.0)
                .with_rotation_euler(std::f32::consts::FRAC_PI_2, 0.0, 0.0),
        );
        world.add_child(parent, target).expect("attach target");

        let world_point = TransformGizmoSystem::target_translation_local_to_world(
            &world,
            target,
            [4.0, 5.0, 6.0],
        );
        approx3(world_point, [4.0, 5.0, 6.0]);

        let local_point = TransformGizmoSystem::world_point_to_target_translation_local(
            &world,
            target,
            [7.0, 8.0, 9.0],
        );
        approx3(local_point, [7.0, 8.0, 9.0]);
    }
}
