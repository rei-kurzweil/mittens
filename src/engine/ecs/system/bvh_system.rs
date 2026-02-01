use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::RenderableComponent;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::{CpuMeshHandle, TransformMatrix};
use crate::engine::user_input::InputState;
use bvh::aabb::{AABB, Bounded};
use bvh::bounding_hierarchy::BHShape;
use bvh::bvh::BVH;
use bvh::bvh::BVHNode;
use bvh::ray::Ray;
use bvh::{Point3, Vector3};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct BvhSystem {
    shapes: Vec<RenderableAabb>,
    bvh: Option<BVH>,
}

#[derive(Debug, Clone)]
struct RenderableAabb {
    component: ComponentId,
    aabb: AABB,
    node_index: usize,
}

impl RenderableAabb {
    fn new(component: ComponentId, min: [f32; 3], max: [f32; 3]) -> Self {
        let min = Point3::new(min[0], min[1], min[2]);
        let max = Point3::new(max[0], max[1], max[2]);
        Self {
            component,
            aabb: AABB::with_bounds(min, max),
            node_index: 0,
        }
    }
}

impl Bounded for RenderableAabb {
    fn aabb(&self) -> AABB {
        self.aabb
    }
}

impl BHShape for RenderableAabb {
    fn set_bh_node_index(&mut self, index: usize) {
        self.node_index = index;
    }

    fn bh_node_index(&self) -> usize {
        self.node_index
    }
}

impl BvhSystem {
    pub fn rebuild_renderable_bvh(&mut self, world: &World) {
        self.shapes.clear();

        for cid in world.all_components() {
            let Some(r) = world.get_component_by_id_as::<RenderableComponent>(cid) else {
                continue;
            };

            // Use base mesh so UV-baked variants (text glyphs) still behave like their primitive.
            let mesh = r.renderable.base_mesh;
            let Some(model) = TransformSystem::world_model(world, cid) else {
                continue;
            };

            let Some((min, max)) = aabb_from_world_matrix_for_mesh(mesh, model) else {
                continue;
            };

            self.shapes.push(RenderableAabb::new(cid, min, max));
        }

        if self.shapes.is_empty() {
            self.bvh = None;
            return;
        }

        // Build the BVH in-place (sets node indices on shapes).
        let mut shapes = std::mem::take(&mut self.shapes);
        let bvh = BVH::build(&mut shapes);
        self.shapes = shapes;
        self.bvh = Some(bvh);
    }

    pub fn raycast_renderables(
        &self,
        origin: [f32; 3],
        dir: [f32; 3],
        max_distance: f32,
    ) -> Option<(ComponentId, f32)> {
        let Some(bvh) = &self.bvh else {
            return None;
        };

        let origin = Point3::new(origin[0], origin[1], origin[2]);
        let dir = Vector3::new(dir[0], dir[1], dir[2]);
        let ray = Ray::new(origin, dir);

        let candidates = bvh.traverse(&ray, &self.shapes);

        let mut best: Option<(ComponentId, f32)> = None;
        for s in candidates {
            let min = [s.aabb.min.x, s.aabb.min.y, s.aabb.min.z];
            let max = [s.aabb.max.x, s.aabb.max.y, s.aabb.max.z];

            let Some(t) = ray_aabb(
                [origin.x, origin.y, origin.z],
                [dir.x, dir.y, dir.z],
                min,
                max,
            ) else {
                continue;
            };

            if t < 0.0 || t > max_distance {
                continue;
            }

            match best {
                None => best = Some((s.component, t)),
                Some((_, bt)) if t < bt => best = Some((s.component, t)),
                _ => {}
            }
        }

        best
    }

    pub fn query_point(&self, point: [f32; 3]) -> Vec<ComponentId> {
        let Some(bvh) = &self.bvh else {
            return Vec::new();
        };

        if self.shapes.is_empty() || bvh.nodes.is_empty() {
            return Vec::new();
        }

        let p = Point3::new(point[0], point[1], point[2]);

        let mut hits = Vec::new();
        let mut stack = vec![0usize];
        while let Some(node_index) = stack.pop() {
            match bvh.nodes[node_index] {
                BVHNode::Node {
                    child_l_index,
                    child_l_aabb,
                    child_r_index,
                    child_r_aabb,
                    ..
                } => {
                    if child_l_aabb.contains(&p) {
                        stack.push(child_l_index);
                    }
                    if child_r_aabb.contains(&p) {
                        stack.push(child_r_index);
                    }
                }
                BVHNode::Leaf { shape_index, .. } => {
                    if let Some(s) = self.shapes.get(shape_index) {
                        if s.aabb.contains(&p) {
                            hits.push(s.component);
                        }
                    }
                }
            }
        }

        hits
    }

    pub fn query_aabb(&self, min: [f32; 3], max: [f32; 3]) -> Vec<ComponentId> {
        let Some(bvh) = &self.bvh else {
            return Vec::new();
        };

        if self.shapes.is_empty() || bvh.nodes.is_empty() {
            return Vec::new();
        }

        let query = AABB::with_bounds(
            Point3::new(min[0], min[1], min[2]),
            Point3::new(max[0], max[1], max[2]),
        );

        let debug = bvh_query_aabb_debug_enabled();
        let mut visited_by_depth: BTreeMap<u32, usize> = BTreeMap::new();
        let mut visited_total: usize = 0;
        let mut visited_leaves: usize = 0;
        let mut max_depth_seen: u32 = 0;

        let mut hits = Vec::new();
        let mut stack = vec![0usize];
        while let Some(node_index) = stack.pop() {
            match bvh.nodes[node_index] {
                BVHNode::Node {
                    child_l_index,
                    child_l_aabb,
                    child_r_index,
                    child_r_aabb,
                    depth,
                    ..
                } => {
                    if debug {
                        visited_total += 1;
                        max_depth_seen = max_depth_seen.max(depth);
                        *visited_by_depth.entry(depth).or_default() += 1;
                    }
                    if aabb_overlap_bvh(&query, &child_l_aabb) {
                        stack.push(child_l_index);
                    }
                    if aabb_overlap_bvh(&query, &child_r_aabb) {
                        stack.push(child_r_index);
                    }
                }
                BVHNode::Leaf {
                    shape_index, depth, ..
                } => {
                    if debug {
                        visited_total += 1;
                        visited_leaves += 1;
                        max_depth_seen = max_depth_seen.max(depth);
                        *visited_by_depth.entry(depth).or_default() += 1;
                    }
                    if let Some(s) = self.shapes.get(shape_index) {
                        if aabb_overlap_bvh(&query, &s.aabb) {
                            hits.push(s.component);
                        }
                    }
                }
            }
        }

        if debug {
            println!(
                "[BvhSystem] query_aabb: min={:?} max={:?} visited_total={} leaves_visited={} hits={} max_depth={}",
                min,
                max,
                visited_total,
                visited_leaves,
                hits.len(),
                max_depth_seen
            );
            for (depth, count) in visited_by_depth {
                println!("[BvhSystem] query_aabb: depth {} visited {}", depth, count);
            }
        }

        hits
    }
}

fn bvh_query_aabb_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_BVH_QUERY_AABB_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn aabb_overlap_bvh(a: &AABB, b: &AABB) -> bool {
    !(a.max.x < b.min.x
        || a.min.x > b.max.x
        || a.max.y < b.min.y
        || a.min.y > b.max.y
        || a.max.z < b.min.z
        || a.min.z > b.max.z)
}

impl crate::engine::ecs::system::System for BvhSystem {
    fn tick(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // For now: rebuild every frame.
        // Later: hook into transform/renderable change events and rebuild only when dirty.
        self.rebuild_renderable_bvh(world);
    }
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

fn aabb_from_world_matrix_for_mesh(
    mesh: CpuMeshHandle,
    m: TransformMatrix,
) -> Option<([f32; 3], [f32; 3])> {
    let (local_pts, thickness) = match mesh {
        CpuMeshHandle::CUBE => (
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
        ),
        CpuMeshHandle::QUAD_2D | CpuMeshHandle::TRIANGLE_2D => (
            vec![
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [-0.5, 0.5, 0.0],
                [0.5, 0.5, 0.0],
            ],
            0.01,
        ),
        _ => return None,
    };

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for p in local_pts {
        let v = [p[0], p[1], p[2], 1.0];
        let w = mat4_mul_vec4(m, v);
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
