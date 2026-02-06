use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::BackgroundComponent;
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
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct BvhSystem {
    shapes: Vec<RenderableAabb>,
    bvh: Option<BVH>,

    /// Map ECS ComponentId -> shape index in `shapes`.
    index_by_component: HashMap<ComponentId, usize>,

    /// Renderables that were registered this frame (via command flush).
    pending_add: Vec<ComponentId>,

    /// Renderables whose AABBs need updating due to transform propagation.
    pending_refit: HashSet<ComponentId>,

    /// Shape indices that need refitting in the BVH.
    pending_refit_shape_indices: HashSet<usize>,

    /// True when shapes were added/removed and we need a full rebuild.
    dirty_rebuild: bool,
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
    pub(crate) fn renderable_is_raycastable(world: &World, renderable_cid: ComponentId) -> bool {
        // Nearest BackgroundComponent ancestor wins.
        // Default: renderables are raycastable unless placed under a background.
        let mut cur = renderable_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(bg) = world.get_component_by_id_as::<BackgroundComponent>(parent) {
                return bg.ray_casting;
            }
            cur = parent;
        }
        true
    }

    pub fn queue_renderable_added(&mut self, component: ComponentId) {
        if self.index_by_component.contains_key(&component) {
            return;
        }
        if self.pending_add.contains(&component) {
            return;
        }
        self.pending_add.push(component);
    }

    pub fn queue_renderable_removed(&mut self, component: ComponentId) {
        // If it's still pending add (not committed to shapes yet), just drop it.
        self.pending_add.retain(|&c| c != component);
        self.pending_refit.remove(&component);

        let Some(index) = self.index_by_component.remove(&component) else {
            return;
        };

        // Remove by swap_remove to keep O(1).
        let last_index = self.shapes.len().saturating_sub(1);
        self.shapes.swap_remove(index);
        if index != last_index {
            // We swapped some other shape into `index`; fix its index mapping.
            if let Some(swapped) = self.shapes.get(index) {
                self.index_by_component.insert(swapped.component, index);
            }
        }

        self.dirty_rebuild = true;
        self.pending_refit_shape_indices.clear();
    }

    /// Queue all renderables under the given transform subtree for BVH refit.
    pub fn queue_transform_subtree(&mut self, world: &World, transform_root: ComponentId) {
        let mut stack = vec![transform_root];
        while let Some(node) = stack.pop() {
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_some()
            {
                self.pending_refit.insert(node);
            }

            let children: Vec<ComponentId> = world.children_of(node).to_vec();
            for ch in children {
                stack.push(ch);
            }
        }
    }

    fn placeholder_aabb() -> AABB {
        // Far away and tiny so it won't get hit.
        let p = 1.0e9_f32;
        AABB::with_bounds(
            Point3::new(p, p, p),
            Point3::new(p + 0.001, p + 0.001, p + 0.001),
        )
    }

    fn compute_aabb_for_renderable(world: &World, cid: ComponentId) -> Option<AABB> {
        let r = world.get_component_by_id_as::<RenderableComponent>(cid)?;

        // Use base mesh so UV-baked variants (text glyphs) still behave like their primitive.
        let mesh = r.renderable.base_mesh;
        let model = TransformSystem::world_model(world, cid)?;
        let (min, max) = aabb_from_world_matrix_for_mesh(mesh, model)?;

        Some(AABB::with_bounds(
            Point3::new(min[0], min[1], min[2]),
            Point3::new(max[0], max[1], max[2]),
        ))
    }

    fn rebuild_from_shapes(&mut self) {
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

    /// Apply any queued add/remove/refit requests.
    ///
    /// Intended to be called once after `CommandQueue::flush` completes.
    pub fn flush_pending(&mut self, world: &World) {
        // Commit pending adds.
        if !self.pending_add.is_empty() {
            for cid in std::mem::take(&mut self.pending_add) {
                if self.index_by_component.contains_key(&cid) {
                    continue;
                }

                if !Self::renderable_is_raycastable(world, cid) {
                    continue;
                }

                let aabb = Self::compute_aabb_for_renderable(world, cid)
                    .unwrap_or_else(Self::placeholder_aabb);

                let idx = self.shapes.len();
                let mut shape = RenderableAabb {
                    component: cid,
                    aabb,
                    node_index: 0,
                };
                // If we already have a BVH, this will be overwritten by rebuild.
                shape.set_bh_node_index(0);

                self.shapes.push(shape);
                self.index_by_component.insert(cid, idx);
            }

            self.dirty_rebuild = true;
        }

        // Update AABBs for moved renderables.
        if !self.pending_refit.is_empty() {
            let moved = std::mem::take(&mut self.pending_refit);
            for cid in moved {
                let Some(&shape_index) = self.index_by_component.get(&cid) else {
                    continue;
                };

                // If the renderable disappeared, drop it.
                if world
                    .get_component_by_id_as::<RenderableComponent>(cid)
                    .is_none()
                {
                    self.queue_renderable_removed(cid);
                    continue;
                }

                let new_aabb = Self::compute_aabb_for_renderable(world, cid)
                    .unwrap_or_else(Self::placeholder_aabb);

                if let Some(s) = self.shapes.get_mut(shape_index) {
                    s.aabb = new_aabb;
                    self.pending_refit_shape_indices.insert(shape_index);
                }
            }
        }

        // Rebuild if topology changed.
        if self.dirty_rebuild {
            // If any shapes were removed via swap_remove, their indices changed; safest is to
            // refit nothing and rebuild.
            self.pending_refit_shape_indices.clear();
            self.rebuild_from_shapes();
            self.dirty_rebuild = false;
            return;
        }

        // Otherwise, update the existing BVH's AABBs and do cheap incremental optimization.
        if !self.pending_refit_shape_indices.is_empty() {
            match self.bvh.as_mut() {
                None => {
                    self.rebuild_from_shapes();
                }
                Some(bvh) => {
                    bvh.optimize(&self.pending_refit_shape_indices, &self.shapes);
                }
            }
            self.pending_refit_shape_indices.clear();
        }
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
                    if aabb_overlap_bvh(&query, &child_l_aabb) {
                        stack.push(child_l_index);
                    }
                    if aabb_overlap_bvh(&query, &child_r_aabb) {
                        stack.push(child_r_index);
                    }
                }
                BVHNode::Leaf { shape_index, .. } => {
                    if let Some(s) = self.shapes.get(shape_index) {
                        if aabb_overlap_bvh(&query, &s.aabb) {
                            hits.push(s.component);
                        }
                    }
                }
            }
        }

        hits
    }
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
        // Event-driven via command queue flush.
        // Keep this as a safety net in case someone mutates transforms/renderables directly.
        let _ = world;
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
