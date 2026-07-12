use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    RayCastComponent, RayCastMode, RaycastableComponent, RaycastableShapeComponent,
    RaycastableShapeType, RenderableComponent,
};
use crate::engine::ecs::system::BvhSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{EventSignal, RxWorld};
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::{CpuMeshHandle, TransformMatrix};
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use winit::event::MouseButton;

#[derive(Debug, Default)]
pub struct RayCastSystem {
    raycasters: HashSet<ComponentId>,
    last_hit: HashMap<ComponentId, Option<ComponentId>>,

    /// Renderables eligible for raycasting, maintained incrementally on renderable add/remove.
    ///
    /// This is used to avoid scanning `world.all_components()` for brute-force fallback tests.
    eligible_renderables: HashSet<ComponentId>,
    profile_frames: u64,
    profile_rays: u64,
    profile_bvh_hits: u64,
    profile_fallbacks: u64,
    profile_fallback_candidates: u64,
    profile_query_time: Duration,
}

#[derive(Debug, Clone, Copy)]
struct CursorRay {
    origin: [f32; 3],
    dir: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaySourceKind {
    CursorThroughActiveCamera,
    ParentForward,
}

// Topology helpers live in pointer_system; use them directly.
use crate::engine::ecs::system::pointer_system::pointer_topology_context;

impl RayCastSystem {
    fn debug_raycast_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            let v = std::env::var("CAT_DEBUG_RAYCAST").unwrap_or_default();
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
    }

    fn profile_spatial_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            matches!(
                std::env::var("CAT_PROFILE_SPATIAL")
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
    }

    fn debug_component_label(world: &World, component: ComponentId) -> String {
        world
            .get_component_record(component)
            .map(|n| {
                if n.name.is_empty() {
                    n.component_type.clone()
                } else {
                    format!("{}: {}", n.component_type, n.name)
                }
            })
            .unwrap_or_else(|| "<missing>".to_string())
    }

    fn raycastable_for_renderable(
        world: &World,
        renderable: ComponentId,
    ) -> Option<RaycastableComponent> {
        BvhSystem::find_raycastable_for_renderable(world, renderable)
    }

    fn sort_hits_by_priority(world: &World, hits: &mut [(ComponentId, f32)]) {
        hits.sort_by(|a, b| {
            let a_pri = Self::raycastable_for_renderable(world, a.0)
                .map(|rc| rc.interaction_priority)
                .unwrap_or(0);
            let b_pri = Self::raycastable_for_renderable(world, b.0)
                .map(|rc| rc.interaction_priority)
                .unwrap_or(0);

            b_pri
                .cmp(&a_pri)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });
    }

    fn explicit_shape_on_renderable(
        world: &World,
        renderable: ComponentId,
    ) -> Option<RaycastableShapeType> {
        world.children_of(renderable).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<RaycastableShapeComponent>(ch)
                .map(|s| s.shape)
        })
    }

    fn infer_shape_from_base_mesh(mesh: CpuMeshHandle) -> RaycastableShapeType {
        match mesh {
            CpuMeshHandle::CUBE => RaycastableShapeType::Box,
            CpuMeshHandle::QUAD_2D => RaycastableShapeType::Quad2D,
            CpuMeshHandle::TRIANGLE_2D => RaycastableShapeType::Triangle2D,
            CpuMeshHandle::TETRAHEDRON => RaycastableShapeType::Tetrahedron,
            CpuMeshHandle::CONE => RaycastableShapeType::Cone,
            CpuMeshHandle::CIRCLE_2D => RaycastableShapeType::Ring2D,
            _ => RaycastableShapeType::Aabb,
        }
    }

    fn resolved_shape_for_renderable(
        world: &World,
        renderable: ComponentId,
    ) -> RaycastableShapeType {
        if let Some(shape) = Self::explicit_shape_on_renderable(world, renderable) {
            if shape != RaycastableShapeType::InferFromBaseMesh {
                return shape;
            }
        }

        let Some(r) = world.get_component_by_id_as::<RenderableComponent>(renderable) else {
            return RaycastableShapeType::Aabb;
        };
        Self::infer_shape_from_base_mesh(r.renderable.base_mesh)
    }

    fn narrow_phase_accept(
        world: &World,
        renderable: ComponentId,
        origin: [f32; 3],
        dir: [f32; 3],
        t_aabb: f32,
    ) -> Option<f32> {
        let shape = Self::resolved_shape_for_renderable(world, renderable);

        // If we can't compute a stable local-space transform, fall back to AABB-only.
        let Some(model) = TransformSystem::world_model(world, renderable) else {
            return Some(t_aabb);
        };
        let Some(inv_model) = math::mat4_inverse(model) else {
            return Some(t_aabb);
        };

        // Transform the ray into renderable-local space.
        let o4 = Self::mat4_mul_vec4(inv_model, [origin[0], origin[1], origin[2], 1.0]);
        let d4 = Self::mat4_mul_vec4(inv_model, [dir[0], dir[1], dir[2], 0.0]);
        let o_local = [o4[0], o4[1], o4[2]];
        let d_local = [d4[0], d4[1], d4[2]];

        fn to_world_t(
            model: TransformMatrix,
            origin_world: [f32; 3],
            dir_world: [f32; 3],
            p_local: [f32; 3],
        ) -> Option<f32> {
            let w = RayCastSystem::mat4_mul_vec4(model, [p_local[0], p_local[1], p_local[2], 1.0]);
            let p_world = [w[0], w[1], w[2]];
            let v = RayCastSystem::vec3_sub(p_world, origin_world);
            let t = RayCastSystem::vec3_dot(v, dir_world);
            if t.is_finite() && t >= 0.0 {
                Some(t)
            } else {
                None
            }
        }

        fn ray_triangle_mt(
            o: [f32; 3],
            d: [f32; 3],
            v0: [f32; 3],
            v1: [f32; 3],
            v2: [f32; 3],
        ) -> Option<f32> {
            // Möller–Trumbore. Returns t along local ray. Accepts both sides (no backface cull).
            let eps = 1e-7_f32;
            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let p = RayCastSystem::vec3_cross(d, e2);
            let det = RayCastSystem::vec3_dot(e1, p);
            if det.abs() < eps {
                return None;
            }
            let inv_det = 1.0 / det;
            let tvec = [o[0] - v0[0], o[1] - v0[1], o[2] - v0[2]];
            let u = RayCastSystem::vec3_dot(tvec, p) * inv_det;
            if !(0.0..=1.0).contains(&u) {
                return None;
            }
            let q = RayCastSystem::vec3_cross(tvec, e1);
            let v = RayCastSystem::vec3_dot(d, q) * inv_det;
            if v < 0.0 || u + v > 1.0 {
                return None;
            }
            let t = RayCastSystem::vec3_dot(e2, q) * inv_det;
            if t.is_finite() && t >= 0.0 {
                Some(t)
            } else {
                None
            }
        }

        fn best_triangle_hit_world_t(
            model: TransformMatrix,
            origin_world: [f32; 3],
            dir_world: [f32; 3],
            o_local: [f32; 3],
            d_local: [f32; 3],
            tris: &[[[f32; 3]; 3]],
        ) -> Option<f32> {
            let mut best: Option<f32> = None;
            for tri in tris {
                let t_local = ray_triangle_mt(o_local, d_local, tri[0], tri[1], tri[2]);
                let Some(t_local) = t_local else {
                    continue;
                };
                let p_local = [
                    o_local[0] + d_local[0] * t_local,
                    o_local[1] + d_local[1] * t_local,
                    o_local[2] + d_local[2] * t_local,
                ];
                let Some(t_world) = to_world_t(model, origin_world, dir_world, p_local) else {
                    continue;
                };
                best = match best {
                    None => Some(t_world),
                    Some(bt) if t_world < bt => Some(t_world),
                    _ => best,
                };
            }
            best
        }

        // Local-space narrow-phase tests.
        match shape {
            RaycastableShapeType::Aabb => Some(t_aabb),

            RaycastableShapeType::Box => {
                let t_local =
                    Self::ray_aabb(o_local, d_local, [-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])?;
                let p_local = [
                    o_local[0] + d_local[0] * t_local,
                    o_local[1] + d_local[1] * t_local,
                    o_local[2] + d_local[2] * t_local,
                ];
                to_world_t(model, origin, dir, p_local)
            }

            RaycastableShapeType::Quad2D => {
                // Keep quad as a slab proxy; triangle is handled precisely below.
                let thick = 0.02_f32;
                let t_local =
                    Self::ray_aabb(o_local, d_local, [-0.5, -0.5, -thick], [0.5, 0.5, thick])?;
                let p_local = [
                    o_local[0] + d_local[0] * t_local,
                    o_local[1] + d_local[1] * t_local,
                    o_local[2] + d_local[2] * t_local,
                ];
                to_world_t(model, origin, dir, p_local)
            }

            RaycastableShapeType::Triangle2D => {
                // Exact triangle test matching MeshFactory::triangle_2d().
                let h = 0.866_025_4_f32;
                let y_top = 2.0 * h / 3.0;
                let y_bottom = -h / 3.0;
                let v0 = [-0.5, y_bottom, 0.0];
                let v1 = [0.5, y_bottom, 0.0];
                let v2 = [0.0, y_top, 0.0];
                let tris: [[[f32; 3]; 3]; 1] = [[v0, v1, v2]];
                best_triangle_hit_world_t(model, origin, dir, o_local, d_local, &tris[..])
            }

            RaycastableShapeType::Ring2D => {
                // Builtin ring mesh is generated as an annulus in the local XY plane.
                // `RenderAssets` uses MeshFactory::circle_2d(0.45, 0.5, ...).
                let mut r_in = 0.45_f32;
                let mut r_out = 0.5_f32;

                // Picking tolerance: slightly widen the clickable band.
                let tol = 0.03_f32;
                r_in = (r_in - tol).max(0.0);
                r_out += tol;

                // Intersect with local plane z=0.
                let dz = d_local[2];
                if dz.abs() < 1e-6 {
                    return None;
                }
                let t_local = -o_local[2] / dz;
                if !t_local.is_finite() || t_local < 0.0 {
                    return None;
                }

                let p_local = [
                    o_local[0] + d_local[0] * t_local,
                    o_local[1] + d_local[1] * t_local,
                    0.0,
                ];
                let r = (p_local[0] * p_local[0] + p_local[1] * p_local[1]).sqrt();
                if r < r_in || r > r_out {
                    return None;
                }
                to_world_t(model, origin, dir, p_local)
            }

            RaycastableShapeType::Cone => {
                // Practical proxy: treat the cone as a finite cylinder in local space.
                // This reduces AABB false positives under rotation, even if it's not a perfect cone.
                let r = 0.5_f32;
                let zmin = -0.5_f32;
                let zmax = 0.5_f32;

                let ox = o_local[0];
                let oy = o_local[1];
                let oz = o_local[2];
                let dx = d_local[0];
                let dy = d_local[1];
                let dz = d_local[2];

                let mut best_t: Option<f32> = None;

                // Body intersection.
                let a = dx * dx + dy * dy;
                let b = 2.0 * (ox * dx + oy * dy);
                let c = ox * ox + oy * oy - r * r;

                if a.abs() > 1e-8 {
                    let disc = b * b - 4.0 * a * c;
                    if disc >= 0.0 {
                        let s = disc.sqrt();
                        let t0 = (-b - s) / (2.0 * a);
                        let t1 = (-b + s) / (2.0 * a);
                        for t in [t0, t1] {
                            if !t.is_finite() || t < 0.0 {
                                continue;
                            }
                            let z = oz + dz * t;
                            if z >= zmin && z <= zmax {
                                best_t = match best_t {
                                    None => Some(t),
                                    Some(bt) if t < bt => Some(t),
                                    _ => best_t,
                                };
                            }
                        }
                    }
                }

                // Caps.
                if dz.abs() > 1e-8 {
                    for z_plane in [zmin, zmax] {
                        let t = (z_plane - oz) / dz;
                        if !t.is_finite() || t < 0.0 {
                            continue;
                        }
                        let x = ox + dx * t;
                        let y = oy + dy * t;
                        if x * x + y * y <= r * r {
                            best_t = match best_t {
                                None => Some(t),
                                Some(bt) if t < bt => Some(t),
                                _ => best_t,
                            };
                        }
                    }
                }

                let t_local = best_t?;
                let p_local = [ox + dx * t_local, oy + dy * t_local, oz + dz * t_local];
                to_world_t(model, origin, dir, p_local)
            }

            RaycastableShapeType::Tetrahedron => {
                // Exact tetrahedron test matching MeshFactory::tetrahedron() indices.
                let p0 = [0.0, 0.0, 0.6123724];
                let p1 = [-0.5, -0.2886751, -0.2041241];
                let p2 = [0.5, -0.2886751, -0.2041241];
                let p3 = [0.0, 0.5773503, -0.2041241];

                // Faces (CCW as authored):
                let tris = [[p0, p1, p2], [p0, p3, p1], [p0, p2, p3], [p1, p3, p2]];
                best_triangle_hit_world_t(model, origin, dir, o_local, d_local, &tris[..])
            }

            RaycastableShapeType::InferFromBaseMesh => Some(t_aabb),
        }
    }
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

    fn should_cast(
        mode: RayCastMode,
        input: &InputState,
        cast_requested: bool,
        source: RaySourceKind,
        pointer_trigger_active: bool,
    ) -> bool {
        match mode {
            RayCastMode::Continuous => true,
            RayCastMode::EventDriven => {
                let desktop_mouse_auto_cast = source == RaySourceKind::CursorThroughActiveCamera
                    && (input.mouse_pressed.contains(&MouseButton::Left)
                        || (input.mouse_down.contains(&MouseButton::Left)
                            && input.mouse_dragging()));

                cast_requested || desktop_mouse_auto_cast || pointer_trigger_active
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
        Some(CursorRay { origin: near, dir })
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

    /// Infer ray source behavior from topology.
    ///
    /// Current runtime policy remains conservative:
    /// - desktop-style pointers use cursor-through-active-camera when their pose lineage contains
    ///   a desktop camera anchor in the same transform lineage
    /// - everything else still uses parent-forward
    ///
    /// We also classify outer driver ancestry here so gesture trigger policy can later prefer a
    /// stronger enclosing driver without requiring the pointer to move in the authored topology.
    fn inferred_source_kind(world: &World, raycaster_cid: ComponentId) -> RaySourceKind {
        let topology = pointer_topology_context(world, raycaster_cid);

        let _future_trigger_policy_hint = (
            topology.has_desktop_input_driver,
            topology.has_xr_input_driver,
            topology.has_controller_driver,
            topology.has_xr_camera_anchor,
        );

        if topology.has_desktop_camera_anchor {
            RaySourceKind::CursorThroughActiveCamera
        } else {
            RaySourceKind::ParentForward
        }
    }

    fn ray_from_parent_forward(
        world: &World,
        raycaster_cid: ComponentId,
    ) -> Option<([f32; 3], [f32; 3])> {
        // World model matrix of nearest ancestor TransformComponent.
        let model = TransformSystem::world_model(world, raycaster_cid)?;
        let origin = [model[3][0], model[3][1], model[3][2]];

        // Engine forward convention is -Z.
        let forward_local = [0.0, 0.0, -1.0];
        let dir_world = Self::vec3_normalize(Self::mat4_mul_vec3_dir(model, forward_local));
        Some((origin, dir_world))
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
            CpuMeshHandle::TETRAHEDRON => (
                vec![
                    [0.0, 0.0, 0.6123724],
                    [-0.5, -0.2886751, -0.2041241],
                    [0.5, -0.2886751, -0.2041241],
                    [0.0, 0.5773503, -0.2041241],
                ],
                0.0,
            ),
            CpuMeshHandle::CONE => (
                vec![
                    // Base circle extremes at z = -0.5 and tip at z = +0.5.
                    [-0.5, 0.0, -0.5],
                    [0.5, 0.0, -0.5],
                    [0.0, -0.5, -0.5],
                    [0.0, 0.5, -0.5],
                    [0.0, 0.0, 0.5],
                ],
                0.0,
            ),
            CpuMeshHandle::QUAD_2D | CpuMeshHandle::TRIANGLE_2D | CpuMeshHandle::CIRCLE_2D => {
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
            CpuMeshHandle::SPHERE => (
                vec![
                    [-0.5, 0.0, 0.0],
                    [0.5, 0.0, 0.0],
                    [0.0, -0.5, 0.0],
                    [0.0, 0.5, 0.0],
                    [0.0, 0.0, -0.5],
                    [0.0, 0.0, 0.5],
                ],
                0.0,
            ),
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

    /// Brute-force all-hits: returns every eligible renderable hit, sorted front-to-back by t.
    fn cast_against_renderables(
        &self,
        world: &World,
        origin: [f32; 3],
        dir: [f32; 3],
        max_distance: f32,
    ) -> Vec<(ComponentId, f32)> {
        let mut hits: Vec<(ComponentId, f32)> = Vec::new();

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

            let Some(t2) = Self::narrow_phase_accept(world, cid, origin, dir, t) else {
                continue;
            };
            if t2 < 0.0 || t2 > max_distance {
                continue;
            }

            hits.push((cid, t2));
        }

        Self::sort_hits_by_priority(world, &mut hits);
        hits
    }

    /// BVH-accelerated all-hits: returns every BVH candidate that passes narrow phase, sorted
    /// front-to-back by t.
    fn cast_against_renderables_bvh(
        &self,
        world: &World,
        bvh: &BvhSystem,
        origin: [f32; 3],
        dir: [f32; 3],
        max_distance: f32,
    ) -> Vec<(ComponentId, f32)> {
        let candidates = bvh.raycast_renderables_candidates(origin, dir, max_distance, 64);

        let mut hits: Vec<(ComponentId, f32)> = Vec::new();
        for (cid, t_aabb) in candidates {
            if world
                .get_component_by_id_as::<RenderableComponent>(cid)
                .is_none()
            {
                continue;
            }

            let Some(t2) = Self::narrow_phase_accept(world, cid, origin, dir, t_aabb) else {
                continue;
            };
            if t2 < 0.0 || t2 > max_distance {
                continue;
            }

            hits.push((cid, t2));
        }

        Self::sort_hits_by_priority(world, &mut hits);
        hits
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
        // integrate with RxWorld for events. If this gets called directly, we can still do hit
        // testing and prints.

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

            let source = Self::inferred_source_kind(world, rcid);

            if !Self::should_cast(rc.mode, input, cast_requested, source, false) {
                continue;
            }

            let hits = self.cast_against_renderables(world, ray.origin, ray.dir, rc.max_distance);
            let best = hits.first().copied();

            match rc.mode {
                RayCastMode::Continuous => {
                    let next = best.map(|(cid, _)| cid);
                    self.last_hit.insert(rcid, next);
                }
                RayCastMode::EventDriven => {}
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
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        rx: &mut RxWorld,
        bvh: &BvhSystem,
        activations: &crate::engine::ecs::system::pointer_system::PointerActivations,
        pointer_system: &crate::engine::ecs::system::pointer_system::PointerSystem,
        _dt_sec: f32,
    ) {
        let profile = Self::profile_spatial_enabled();
        if profile {
            self.profile_frames += 1;
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

                // A pointer's trigger being down should auto-cast the same way mouse-down does.
                let pointer_trigger_active = pointer_system
                    .raycast_to_pointer(rcid)
                    .map(|ptr| {
                        activations.down.contains(&ptr) || activations.pressed.contains(&ptr)
                    })
                    .unwrap_or(false);

                let (origin, dir) = match source {
                    RaySourceKind::CursorThroughActiveCamera => {
                        let Some(r) = cursor_ray else {
                            continue;
                        };
                        (r.origin, r.dir)
                    }
                    RaySourceKind::ParentForward => {
                        let Some((o, d)) = Self::ray_from_parent_forward(world, rcid) else {
                            continue;
                        };
                        (o, d)
                    }
                };

                if !Self::should_cast(mode, input, cast_requested, source, pointer_trigger_active) {
                    continue;
                }

                let query_started = profile.then(Instant::now);
                let mut hits = self.cast_against_renderables_bvh(world, bvh, origin, dir, max_distance);
                if profile {
                    self.profile_rays += 1;
                    self.profile_bvh_hits += hits.len() as u64;
                }
                if hits.is_empty() && !bvh.has_index() {
                    hits = self.cast_against_renderables(world, origin, dir, max_distance);
                    if profile {
                        self.profile_fallbacks += 1;
                        self.profile_fallback_candidates += self.eligible_renderables.len() as u64;
                    }
                }
                if let Some(started) = query_started {
                    self.profile_query_time += started.elapsed();
                }
                if Self::debug_raycast_enabled() {
                    let summary: Vec<String> = hits
                        .iter()
                        .take(8)
                        .map(|(cid, t)| {
                            let rc = Self::raycastable_for_renderable(world, *cid);
                            format!(
                                "{:?} '{}' t={:.3} pri={} pe={:?}",
                                cid,
                                Self::debug_component_label(world, *cid),
                                t,
                                rc.map(|r| r.interaction_priority).unwrap_or(0),
                                rc.map(|r| r.pointer_events)
                                    .unwrap_or(crate::engine::ecs::component::PointerEvents::All)
                            )
                        })
                        .collect();
                    eprintln!(
                        "[raycast] rc={:?} source={:?} origin=[{:+.3},{:+.3},{:+.3}] dir=[{:+.3},{:+.3},{:+.3}] hits={}",
                        rcid,
                        source,
                        origin[0],
                        origin[1],
                        origin[2],
                        dir[0],
                        dir[1],
                        dir[2],
                        if summary.is_empty() {
                            "<none>".to_string()
                        } else {
                            summary.join(" | ")
                        }
                    );
                }

                // Emit one RayIntersected per hit (front-to-back). GestureSystem accumulates
                // all of them within the frame and picks drag/click targets independently.
                for &(hit_cid, t) in &hits {
                    rx.push_event(
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

                let best = hits.first().copied();
                match mode {
                    RayCastMode::Continuous => {
                        let next = best.map(|(cid, _)| cid);
                        self.last_hit.insert(rcid, next);
                    }
                    RayCastMode::EventDriven => {}
                }

                if cast_requested {
                    if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(rcid) {
                        rc.cast_requests = 0;
                    }
                }
            }
        }

        if profile && self.profile_frames >= 120 {
            let line = format!(
                "[spatial-profile][raycast] frames={} rays={} bvh_hits={} fallbacks={} fallback_scan_candidates={} query_ms={:.3}",
                self.profile_frames, self.profile_rays, self.profile_bvh_hits,
                self.profile_fallbacks, self.profile_fallback_candidates,
                self.profile_query_time.as_secs_f64() * 1000.0
            );
            eprintln!("{line}");
            crate::utils::profile_log::append("spatial-profile", &line);
            self.profile_frames = 0;
            self.profile_rays = 0;
            self.profile_bvh_hits = 0;
            self.profile_fallbacks = 0;
            self.profile_fallback_candidates = 0;
            self.profile_query_time = Duration::ZERO;
        }
    }
}
