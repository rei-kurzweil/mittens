use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::{EventSignal, RxWorld};
use crate::engine::ecs::component::{
    ColorComponent, RayCastComponent, RayCastMode, RaycastableComponent, RenderableComponent,
};
use crate::engine::ecs::system::BvhSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::{CpuMeshHandle, TransformMatrix};
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::{HashMap, HashSet};
use winit::event::MouseButton;

#[derive(Debug, Default)]
pub struct RayCastSystem {
    raycasters: HashSet<ComponentId>,
    last_hit: HashMap<ComponentId, Option<ComponentId>>,
    debug_left_down_prev: bool,

    /// Renderables eligible for raycasting, maintained incrementally on renderable add/remove.
    ///
    /// This is used to avoid scanning `world.all_components()` for brute-force fallback tests.
    eligible_renderables: HashSet<ComponentId>,

    // Debug ray visualization: raycaster component -> visual root TransformComponent.
    ray_visual_by_raycast: HashMap<ComponentId, ComponentId>,

    // Debug highlight: currently-highlighted renderable (glyph).
    highlighted_renderable: Option<ComponentId>,
    highlighted_color_component: Option<ComponentId>,
}

#[derive(Debug, Clone, Copy)]
struct CursorRay {
    x_ndc: f32,
    y_ndc: f32,
    near: [f32; 3],
    far: [f32; 3],
    origin: [f32; 3],
    dir: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaySourceKind {
    CursorThroughActiveCamera,
    ParentForward,
}

impl RayCastSystem {
    pub fn register_raycast(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        if world
            .get_component_by_id_as::<RayCastComponent>(component)
            .is_none()
        {
            return;
        }
        self.raycasters.insert(component);
    }

    pub fn remove_raycast(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.raycasters.remove(&component);
        self.last_hit.remove(&component);
    }

    pub fn notify_renderable_added(&mut self, world: &World, renderable_cid: ComponentId) {
        if BvhSystem::renderable_is_raycastable(world, renderable_cid) {
            self.eligible_renderables.insert(renderable_cid);
        } else {
            self.eligible_renderables.remove(&renderable_cid);
        }
    }

    pub fn notify_renderable_removed(&mut self, renderable_cid: ComponentId) {
        self.eligible_renderables.remove(&renderable_cid);
    }

    fn should_cast(mode: RayCastMode, input: &InputState, cast_requested: bool) -> bool {
        match mode {
            RayCastMode::Continuous => true,
            RayCastMode::EventDriven => {
                cast_requested || input.mouse_pressed.contains(&MouseButton::Left)
            }
        }
    }

    fn mat4_mul(a: TransformMatrix, b: TransformMatrix) -> TransformMatrix {
        // Column-major mat4 multiplication: out = a * b.
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] =
                    a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
            }
        }
        out
    }

    fn mat4_mul_vec4(m: TransformMatrix, v: [f32; 4]) -> [f32; 4] {
        // Column-major mat4 * vec4.
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
            m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
        ]
    }

    fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn vec3_mul_scalar(v: [f32; 3], s: f32) -> [f32; 3] {
        [v[0] * s, v[1] * s, v[2] * s]
    }

    fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn upsert_renderable_color(
        &mut self,
        world: &mut World,
        queue: &mut crate::engine::ecs::CommandQueue,
        renderable_cid: ComponentId,
        rgba: [f32; 4],
    ) -> Option<ComponentId> {
        // Find an existing ColorComponent directly under the renderable.
        let existing = world
            .children_of(renderable_cid)
            .iter()
            .copied()
            .find(|&ch| world.get_component_by_id_as::<ColorComponent>(ch).is_some());

        let color_cid = match existing {
            Some(cid) => cid,
            None => {
                let cid =
                    world.add_component(ColorComponent::rgba(rgba[0], rgba[1], rgba[2], rgba[3]));
                let _ = world.add_child(renderable_cid, cid);
                cid
            }
        };

        if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
            c.rgba = rgba;
        }

        // Apply immediately via the existing color registration path.
        queue.queue_register_color(color_cid);
        Some(color_cid)
    }

    fn vec3_len(v: [f32; 3]) -> f32 {
        Self::vec3_dot(v, v).sqrt()
    }

    fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
        let len = Self::vec3_len(v);
        if len > 0.0 {
            Self::vec3_mul_scalar(v, 1.0 / len)
        } else {
            [0.0, 0.0, 1.0]
        }
    }

    fn ray_from_cursor(visuals: &VisualWorld, input: &InputState) -> Option<CursorRay> {
        let vp = visuals.viewport();
        let w = vp[0];
        let h = vp[1];
        if w <= 0.0 || h <= 0.0 {
            return None;
        }

        let (cx, cy) = input.cursor_pos.unwrap_or((w * 0.5, h * 0.5));

        // NDC in Vulkan: x in [-1,1], y in [-1,1] with +y up, z in [0,1].
        let x_ndc = (2.0 * (cx / w)) - 1.0;
        let y_ndc = 1.0 - (2.0 * (cy / h));

        let view = visuals.camera_view();
        let proj = visuals.camera_proj();
        let vp_mat = Self::mat4_mul(proj, view);
        let inv_vp = math::mat4_inverse(vp_mat)?;

        let near_clip = [x_ndc, y_ndc, 0.0, 1.0];
        let far_clip = [x_ndc, y_ndc, 1.0, 1.0];

        let near_world4 = Self::mat4_mul_vec4(inv_vp, near_clip);
        let far_world4 = Self::mat4_mul_vec4(inv_vp, far_clip);

        let near_w = near_world4[3];
        let far_w = far_world4[3];
        if near_w == 0.0 || far_w == 0.0 {
            return None;
        }

        let near = [
            near_world4[0] / near_w,
            near_world4[1] / near_w,
            near_world4[2] / near_w,
        ];
        let far = [
            far_world4[0] / far_w,
            far_world4[1] / far_w,
            far_world4[2] / far_w,
        ];

        let dir = Self::vec3_normalize(Self::vec3_sub(far, near));
        Some(CursorRay {
            x_ndc,
            y_ndc,
            near,
            far,
            origin: near,
            dir,
        })
    }

    fn mat4_mul_vec3_dir(m: TransformMatrix, v: [f32; 3]) -> [f32; 3] {
        // Treat v as a direction (w=0).
        let w = Self::mat4_mul_vec4(m, [v[0], v[1], v[2], 0.0]);
        [w[0], w[1], w[2]]
    }

    fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
        let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        if len > 0.0 {
            [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    /// Quaternion rotating vector `from` to vector `to`.
    ///
    /// Both vectors are treated as directions.
    fn quat_from_to(from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
        let f = Self::vec3_normalize(from);
        let t = Self::vec3_normalize(to);
        let dot = Self::vec3_dot(f, t);

        // If vectors are nearly opposite, pick an arbitrary orthogonal axis.
        if dot < -0.999_999 {
            let axis_seed = if f[0].abs() < 0.1 && f[2].abs() < 0.1 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let axis = Self::vec3_normalize(Self::vec3_cross(f, axis_seed));
            return [axis[0], axis[1], axis[2], 0.0];
        }

        let c = Self::vec3_cross(f, t);
        let q = [c[0], c[1], c[2], 1.0 + dot];
        Self::quat_normalize(q)
    }

    fn nearest_ancestor_transform(world: &World, start: ComponentId) -> Option<ComponentId> {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(start)
            .is_some()
        {
            return Some(start);
        }

        let mut cur = start;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
                .is_some()
            {
                return Some(parent);
            }
            cur = parent;
        }
        None
    }

    fn transform_has_camera_child(world: &World, transform_cid: ComponentId) -> bool {
        world.children_of(transform_cid).iter().any(|&ch| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::Camera3DComponent>(ch)
                .is_some()
                || world
                    .get_component_by_id_as::<crate::engine::ecs::component::Camera2DComponent>(ch)
                    .is_some()
        })
    }

    /// Infer ray source behavior from topology:
    /// - If the nearest ancestor TransformComponent also owns a camera component, use cursor->camera ray.
    /// - Otherwise, cast forward (-Z) from that transform's world pose.
    fn inferred_source_kind(world: &World, raycaster_cid: ComponentId) -> RaySourceKind {
        let Some(tcid) = Self::nearest_ancestor_transform(world, raycaster_cid) else {
            return RaySourceKind::CursorThroughActiveCamera;
        };
        if Self::transform_has_camera_child(world, tcid) {
            RaySourceKind::CursorThroughActiveCamera
        } else {
            RaySourceKind::ParentForward
        }
    }

    fn ray_from_parent_forward(world: &World, raycaster_cid: ComponentId) -> Option<([f32; 3], [f32; 3])> {
        // World model matrix of nearest ancestor TransformComponent.
        let model = TransformSystem::world_model(world, raycaster_cid)?;
        let origin = [model[3][0], model[3][1], model[3][2]];

        // Engine forward convention is -Z.
        let forward_local = [0.0, 0.0, -1.0];
        let dir_world = Self::vec3_normalize(Self::mat4_mul_vec3_dir(model, forward_local));
        Some((origin, dir_world))
    }

    fn ensure_ray_visual(
        &mut self,
        world: &mut World,
        queue: &mut crate::engine::ecs::CommandQueue,
        raycaster_cid: ComponentId,
    ) -> Option<ComponentId> {
        let parent = Self::nearest_ancestor_transform(world, raycaster_cid)?;

        if let Some(&vis_root) = self.ray_visual_by_raycast.get(&raycaster_cid) {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(vis_root)
                .is_some()
            {
                // If the raycaster was reparented, keep the visual under the same inferred parent
                // transform so the attachment change is visible in topology.
                if world.parent_of(vis_root) != Some(parent) {
                    let _ = world.add_child(parent, vis_root);

                    if world.is_initialized(parent) && !world.is_initialized(vis_root) {
                        world.init_component_tree(vis_root, queue);
                    }
                }
                return Some(vis_root);
            }
        }

        // Attach the visual under the nearest ancestor transform, so it lives in a reasonable place
        // in the component tree and is initialized if the parent is initialized.
        let vis_t = world.register(
            crate::engine::ecs::component::TransformComponent::new()
                .with_scale(0.02, 0.02, 1.0),
        );
        let vis_r = world.register(
            RenderableComponent::cube(),
        );
        let vis_rc = world.register(RaycastableComponent::disabled());
        let vis_c = world.register(ColorComponent::rgba(1.0, 0.9, 0.2, 1.0));
        let vis_e = world.register(crate::engine::ecs::component::EmissiveComponent::on());

        let _ = world.add_child(parent, vis_t);
        let _ = world.add_child(vis_t, vis_r);
        let _ = world.add_child(vis_r, vis_rc);
        let _ = world.add_child(vis_r, vis_c);
        let _ = world.add_child(vis_r, vis_e);

        if world.is_initialized(parent) {
            world.init_component_tree(vis_t, queue);
        }

        self.ray_visual_by_raycast.insert(raycaster_cid, vis_t);
        Some(vis_t)
    }

    fn update_ray_visual(
        &mut self,
        world: &mut World,
        queue: &mut crate::engine::ecs::CommandQueue,
        raycaster_cid: ComponentId,
        origin: [f32; 3],
        dir: [f32; 3],
        length: f32,
        source: RaySourceKind,
    ) {
        let Some(vis_t) = self.ensure_ray_visual(world, queue, raycaster_cid) else {
            return;
        };

        // Convert the world-space ray (origin/dir) into the local space of the transform that owns
        // the raycaster. The visual is parented under that transform, so its TransformComponent
        // fields must be expressed in parent-local space.
        let Some(parent) = Self::nearest_ancestor_transform(world, raycaster_cid) else {
            return;
        };
        let Some(parent_model) = TransformSystem::world_model(world, parent) else {
            return;
        };
        let Some(inv_parent_model) = math::mat4_inverse(parent_model) else {
            return;
        };

        let o4 = Self::mat4_mul_vec4(inv_parent_model, [origin[0], origin[1], origin[2], 1.0]);
        let d4 = Self::mat4_mul_vec4(inv_parent_model, [dir[0], dir[1], dir[2], 0.0]);

        let origin_local = [o4[0], o4[1], o4[2]];
        let dir_local = Self::vec3_normalize([d4[0], d4[1], d4[2]]);

        let thickness = 0.02;
        let len = length.max(0.01);
        let center = Self::vec3_add(origin_local, Self::vec3_mul_scalar(dir_local, 0.5 * len));

        // Orient the cube's +Z axis along the ray direction.
        let rot = Self::quat_from_to([0.0, 0.0, 1.0], dir_local);

        if let Some(t) = world.get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(vis_t) {
            t.transform.translation = center;
            t.transform.rotation = rot;
            t.transform.scale = [thickness, thickness, len];
            t.transform.recompute_model();
            queue.queue_update_transform(vis_t, t.transform);
        }

        // Color-code based on inferred source.
        let desired = match source {
            RaySourceKind::CursorThroughActiveCamera => [0.2, 0.8, 1.0, 1.0],
            RaySourceKind::ParentForward => [1.0, 0.85, 0.2, 1.0],
        };

        // Find the ColorComponent under the ray visual renderable.
        // Topology: vis_t -> renderable -> (color, emissive)
        if let Some(&renderable_cid) = world.children_of(vis_t).iter().find(|&&ch| {
            world.get_component_by_id_as::<RenderableComponent>(ch).is_some()
        }) {
            if let Some(&color_cid) = world.children_of(renderable_cid).iter().find(|&&ch| {
                world.get_component_by_id_as::<ColorComponent>(ch).is_some()
            }) {
                if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
                    if c.rgba != desired {
                        c.rgba = desired;
                        queue.queue_register_color(color_cid);
                    }
                }
            }
        }
    }

    fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }

    fn aabb_from_world_matrix_for_mesh(
        mesh: CpuMeshHandle,
        m: TransformMatrix,
    ) -> Option<([f32; 3], [f32; 3])> {
        let (local_pts, thickness) = match mesh {
            CpuMeshHandle::CUBE => {
                // Unit cube centered at origin.
                (
                    vec![
                        [-0.5, -0.5, -0.5],
                        [0.5, -0.5, -0.5],
                        [-0.5, 0.5, -0.5],
                        [0.5, 0.5, -0.5],
                        [-0.5, -0.5, 0.5],
                        [0.5, -0.5, 0.5],
                        [-0.5, 0.5, 0.5],
                        [0.5, 0.5, 0.5],
                    ],
                    0.0,
                )
            }
            CpuMeshHandle::QUAD_2D | CpuMeshHandle::TRIANGLE_2D => {
                // Flat meshes live in XY plane. Give them a tiny thickness so AABB tests work.
                (
                    vec![
                        [-0.5, -0.5, 0.0],
                        [0.5, -0.5, 0.0],
                        [-0.5, 0.5, 0.0],
                        [0.5, 0.5, 0.0],
                    ],
                    0.01,
                )
            }
            _ => return None,
        };

        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];

        for p in local_pts {
            let v = [p[0], p[1], p[2], 1.0];
            let w = Self::mat4_mul_vec4(m, v);
            let wp = [w[0], w[1], w[2]];
            for i in 0..3 {
                min[i] = min[i].min(wp[i]);
                max[i] = max[i].max(wp[i]);
            }
        }

        if thickness > 0.0 {
            min[2] -= thickness;
            max[2] += thickness;
        }

        Some((min, max))
    }

    fn ray_aabb(
        origin: [f32; 3],
        dir: [f32; 3],
        aabb_min: [f32; 3],
        aabb_max: [f32; 3],
    ) -> Option<f32> {
        // Slab test. Returns nearest positive t.
        let mut tmin = 0.0f32;
        let mut tmax = f32::INFINITY;

        for axis in 0..3 {
            let o = origin[axis];
            let d = dir[axis];
            let min = aabb_min[axis];
            let max = aabb_max[axis];

            if d.abs() < 1e-6 {
                if o < min || o > max {
                    return None;
                }
                continue;
            }

            let inv_d = 1.0 / d;
            let mut t0 = (min - o) * inv_d;
            let mut t1 = (max - o) * inv_d;
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
            }
            tmin = tmin.max(t0);
            tmax = tmax.min(t1);
            if tmax < tmin {
                return None;
            }
        }

        if tmin >= 0.0 {
            Some(tmin)
        } else if tmax >= 0.0 {
            Some(tmax)
        } else {
            None
        }
    }

    fn cast_against_renderables(
        &self,
        world: &World,
        origin: [f32; 3],
        dir: [f32; 3],
        max_distance: f32,
    ) -> Option<(ComponentId, f32)> {
        let mut best: Option<(ComponentId, f32)> = None;

        for &cid in self.eligible_renderables.iter() {
            let Some(r) = world.get_component_by_id_as::<RenderableComponent>(cid) else {
                continue;
            };

            let mesh = r.renderable.base_mesh;
            let Some(model) = TransformSystem::world_model(world, cid) else {
                continue;
            };

            let Some((min, max)) = Self::aabb_from_world_matrix_for_mesh(mesh, model) else {
                continue;
            };

            let Some(t) = Self::ray_aabb(origin, dir, min, max) else {
                continue;
            };

            if t < 0.0 || t > max_distance {
                continue;
            }

            match best {
                None => best = Some((cid, t)),
                Some((_, bt)) if t < bt => best = Some((cid, t)),
                _ => {}
            }
        }

        best
    }

    fn cast_against_renderables_bvh(
        &self,
        world: &World,
        bvh: &BvhSystem,
        origin: [f32; 3],
        dir: [f32; 3],
        max_distance: f32,
    ) -> Option<(ComponentId, f32)> {
        let hit = bvh.raycast_renderables(origin, dir, max_distance);
        match hit {
            Some((cid, t))
                if world
                    .get_component_by_id_as::<RenderableComponent>(cid)
                    .is_some() =>
            {
                Some((cid, t))
            }
            _ => None,
        }
    }
}

impl System for RayCastSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        _dt_sec: f32,
    ) {
        // NOTE: RayCastSystem is normally ticked via `tick_with_queue` from SystemWorld so it can
        // apply queued side effects (e.g., click highlight color upserts). If this gets called
        // directly, we can still do hit testing and prints.

        if self.raycasters.is_empty() {
            return;
        }

        let Some(ray) = Self::ray_from_cursor(visuals, input) else {
            return;
        };

        // Iterate over a stable snapshot so removal during iteration is safe.
        let raycasters: Vec<ComponentId> = self.raycasters.iter().copied().collect();
        for rcid in raycasters {
            let Some(rc) = world.get_component_by_id_as::<RayCastComponent>(rcid) else {
                self.raycasters.remove(&rcid);
                self.last_hit.remove(&rcid);
                continue;
            };

            let cast_requested = rc.cast_requests > 0;

            if !Self::should_cast(rc.mode, input, cast_requested) {
                continue;
            }

            // Extra debug on click: dump the ray + camera position so we can sanity check.
            if rc.mode == RayCastMode::EventDriven
                && input.mouse_pressed.contains(&MouseButton::Left)
            {
                let view = visuals.camera_view();
                let cam_pos = math::mat4_inverse(view)
                    .map(|inv_view| {
                        let t = inv_view[3];
                        [t[0], t[1], t[2]]
                    })
                    .unwrap_or([f32::NAN, f32::NAN, f32::NAN]);

                println!(
                    "[RayCast] ray debug: cursor={:?} ndc=({:.3},{:.3}) cam_pos=({:.3},{:.3},{:.3}) origin=({:.3},{:.3},{:.3}) dir=({:.3},{:.3},{:.3}) near=({:.3},{:.3},{:.3}) far=({:.3},{:.3},{:.3})",
                    input.cursor_pos,
                    ray.x_ndc,
                    ray.y_ndc,
                    cam_pos[0],
                    cam_pos[1],
                    cam_pos[2],
                    ray.origin[0],
                    ray.origin[1],
                    ray.origin[2],
                    ray.dir[0],
                    ray.dir[1],
                    ray.dir[2],
                    ray.near[0],
                    ray.near[1],
                    ray.near[2],
                    ray.far[0],
                    ray.far[1],
                    ray.far[2]
                );
            }

            let hit = self.cast_against_renderables(world, ray.origin, ray.dir, rc.max_distance);

            match rc.mode {
                RayCastMode::Continuous => {
                    let prev = self.last_hit.get(&rcid).copied().flatten();
                    let next = hit.map(|(cid, _)| cid);
                    if prev != next {
                        if let Some((hit_cid, t)) = hit {
                            let parent = world.parent_of(hit_cid);
                            println!(
                                "[RayCast] hit renderable={:?} parent={:?} t={:.3}",
                                hit_cid, parent, t
                            );
                        } else {
                            println!("[RayCast] no hit");
                        }
                    }
                    self.last_hit.insert(rcid, next);
                }
                RayCastMode::EventDriven => {
                    if let Some((hit_cid, t)) = hit {
                        let parent = world.parent_of(hit_cid);
                        println!(
                            "[RayCast] click hit renderable={:?} parent={:?} t={:.3}",
                            hit_cid, parent, t
                        );
                    } else {
                        println!("[RayCast] click no hit");
                    }
                }
            }

            if cast_requested {
                if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(rcid) {
                    rc.cast_requests = 0;
                }
            }
        }
    }
}

impl RayCastSystem {
    fn inherited_color_rgba(world: &World, start: ComponentId) -> Option<[f32; 4]> {
        let mut cur = start;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(rgba) = world.children_of(parent).iter().find_map(|&ch| {
                world
                    .get_component_by_id_as::<ColorComponent>(ch)
                    .map(|c| c.rgba)
            }) {
                return Some(rgba);
            }
            cur = parent;
        }
        None
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        rx: &mut RxWorld,
        bvh: &BvhSystem,
        _dt_sec: f32,
    ) {
        // Equivalent to `tick()` but uses BVH for hit testing and can apply queued side effects.
        // Keep the debug prints so it's easy to see input edges.
        let left_down = input.mouse_down.contains(&MouseButton::Left);

        if input.mouse_pressed.contains(&MouseButton::Left) {
            println!(
                "[RayCast] debug: left pressed cursor={:?} down={:?}",
                input.cursor_pos, left_down
            );
        }
        if input.mouse_released.contains(&MouseButton::Left) {
            println!(
                "[RayCast] debug: left released cursor={:?} down={:?}",
                input.cursor_pos, left_down
            );
        }

        // Cursor ray (if needed by any raycaster this frame).
        let cursor_ray = Self::ray_from_cursor(visuals, input);

        if !self.raycasters.is_empty() {
            // Iterate over a stable snapshot so removal during iteration is safe.
            let raycasters: Vec<ComponentId> = self.raycasters.iter().copied().collect();
            for rcid in raycasters {
                let Some(rc) = world.get_component_by_id_as::<RayCastComponent>(rcid) else {
                    self.raycasters.remove(&rcid);
                    self.last_hit.remove(&rcid);
                    continue;
                };

                // Copy out what we need so we can mutably borrow `world` later.
                let mode = rc.mode;
                let max_distance = rc.max_distance;
                let cast_requested = rc.cast_requests > 0;

                let source = Self::inferred_source_kind(world, rcid);

                let (origin, dir, debug_cursor) = match source {
                    RaySourceKind::CursorThroughActiveCamera => {
                        let Some(r) = cursor_ray else {
                            continue;
                        };
                        (r.origin, r.dir, Some(r))
                    }
                    RaySourceKind::ParentForward => {
                        let Some((o, d)) = Self::ray_from_parent_forward(world, rcid) else {
                            continue;
                        };
                        (o, d, None)
                    }
                };

                // Always keep the debug ray visual updated so you can see where it will cast.
                self.update_ray_visual(world, queue, rcid, origin, dir, max_distance, source);

                if !Self::should_cast(mode, input, cast_requested) {
                    continue;
                }

                let click_cast = input.mouse_pressed.contains(&MouseButton::Left);
                let action_cast = cast_requested && !click_cast;

                // Extra debug on click.
                if mode == RayCastMode::EventDriven
                    && input.mouse_pressed.contains(&MouseButton::Left)
                {
                    let view = visuals.camera_view();
                    let cam_pos = math::mat4_inverse(view)
                        .map(|inv_view| {
                            let t = inv_view[3];
                            [t[0], t[1], t[2]]
                        })
                        .unwrap_or([f32::NAN, f32::NAN, f32::NAN]);

                    match debug_cursor {
                        Some(ray) => {
                            println!(
                                "[RayCast] ray debug (cursor): cursor={:?} ndc=({:.3},{:.3}) cam_pos=({:.3},{:.3},{:.3}) origin=({:.3},{:.3},{:.3}) dir=({:.3},{:.3},{:.3}) near=({:.3},{:.3},{:.3}) far=({:.3},{:.3},{:.3})",
                                input.cursor_pos,
                                ray.x_ndc,
                                ray.y_ndc,
                                cam_pos[0],
                                cam_pos[1],
                                cam_pos[2],
                                origin[0],
                                origin[1],
                                origin[2],
                                dir[0],
                                dir[1],
                                dir[2],
                                ray.near[0],
                                ray.near[1],
                                ray.near[2],
                                ray.far[0],
                                ray.far[1],
                                ray.far[2]
                            );
                        }
                        None => {
                            println!(
                                "[RayCast] ray debug (parent_forward): origin=({:.3},{:.3},{:.3}) dir=({:.3},{:.3},{:.3}) cam_pos=({:.3},{:.3},{:.3})",
                                origin[0],
                                origin[1],
                                origin[2],
                                dir[0],
                                dir[1],
                                dir[2],
                                cam_pos[0],
                                cam_pos[1],
                                cam_pos[2],
                            );
                        }
                    }
                }

                let hit = self
                    .cast_against_renderables_bvh(world, bvh, origin, dir, max_distance)
                    .or_else(|| self.cast_against_renderables(world, origin, dir, max_distance));

                if let Some((hit_cid, t)) = hit {
                    // Scope the interaction to the intersected renderable so listeners can
                    // subscribe at any ancestor (e.g. a ring root transform).
                    rx.push(
                        hit_cid,
                        EventSignal::RayIntersected {
                            raycaster: rcid,
                            renderable: hit_cid,
                            t,
                            origin,
                            dir,
                        },
                    );
                }

                match mode {
                    RayCastMode::Continuous => {
                        let prev = self.last_hit.get(&rcid).copied().flatten();
                        let next = hit.map(|(cid, _)| cid);
                        if prev != next {
                            if let Some((hit_cid, t)) = hit {
                                let parent = world.parent_of(hit_cid);
                                println!(
                                    "[RayCast] hit renderable={:?} parent={:?} t={:.3}",
                                    hit_cid, parent, t
                                );
                            } else {
                                println!("[RayCast] no hit");
                            }
                        }
                        self.last_hit.insert(rcid, next);
                    }
                    RayCastMode::EventDriven => {
                        if let Some((hit_cid, t)) = hit {
                            let parent = world.parent_of(hit_cid);
                            println!(
                                "[RayCast] {} hit renderable={:?} parent={:?} t={:.3}",
                                if action_cast { "action" } else { "click" },
                                hit_cid,
                                parent,
                                t
                            );
                        } else {
                            println!(
                                "[RayCast] {} no hit",
                                if action_cast { "action" } else { "click" }
                            );
                        }
                    }
                }

                if cast_requested {
                    if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(rcid) {
                        rc.cast_requests = 0;
                    }
                }
            }
        }

        // Restore highlight when the click ends.
        if input.mouse_released.contains(&MouseButton::Left) {
            if let (Some(rid), Some(cid)) = (
                self.highlighted_renderable,
                self.highlighted_color_component,
            ) {
                if world
                    .get_component_by_id_as::<RenderableComponent>(rid)
                    .is_some()
                    && world
                        .get_component_by_id_as::<ColorComponent>(cid)
                        .is_some()
                {
                    let restore_rgba =
                        Self::inherited_color_rgba(world, rid).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(cid) {
                        c.rgba = restore_rgba;
                    }
                    queue.queue_register_color(cid);
                }
            }
            self.highlighted_renderable = None;
            self.highlighted_color_component = None;
        }

        // Click highlight: highlight the renderable under the cursor until mouse release.
        if self.raycasters.is_empty() {
            return;
        }
        if !input.mouse_pressed.contains(&MouseButton::Left) {
            return;
        }
        // For highlight, use the *first* raycaster's inferred ray.
        let mut highlight_ray: Option<([f32; 3], [f32; 3], f32)> = None;

        for &rcid in self.raycasters.iter() {
            if let Some(rc) = world.get_component_by_id_as::<RayCastComponent>(rcid) {
                let source = Self::inferred_source_kind(world, rcid);
                match source {
                    RaySourceKind::CursorThroughActiveCamera => {
                        let Some(r) = Self::ray_from_cursor(visuals, input) else {
                            continue;
                        };
                        highlight_ray = Some((r.origin, r.dir, rc.max_distance));
                        break;
                    }
                    RaySourceKind::ParentForward => {
                        let Some((o, d)) = Self::ray_from_parent_forward(world, rcid) else {
                            continue;
                        };
                        highlight_ray = Some((o, d, rc.max_distance));
                        break;
                    }
                }
            }
        }

        let Some((origin, dir, length)) = highlight_ray else {
            return;
        };

        if let Some((hit_cid, _t)) = self
            .cast_against_renderables_bvh(world, bvh, origin, dir, length)
            .or_else(|| self.cast_against_renderables(world, origin, dir, length))
        {
            let green = [0.2, 1.0, 0.2, 1.0];
            if let Some(color_cid) = self.upsert_renderable_color(world, queue, hit_cid, green) {
                self.highlighted_renderable = Some(hit_cid);
                self.highlighted_color_component = Some(color_cid);
            }
        }
    }
}
