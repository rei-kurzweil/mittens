//! Snapshot-only BVH extraction and batched wire geometry. Never called to
//! maintain the diagnostic between explicit editor requests.
use crate::engine::ecs::component::EditorUIComponent;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    bounds::Aabb,
    mesh::{CpuMesh, CpuVertex},
};
use bvh::{
    bounding_hierarchy::BHShape,
    bvh::{BVH, BVHNode},
};
use std::collections::HashSet;
pub const MAX_BOXES: usize = 4096;
// Capture metadata separately from the draw budget so a full early subtree
// cannot hide all later regions. Fine detail is coarsened after collection.
const MAX_CAPTURE_NODES: usize = 131_072;
#[derive(Debug, Default)]
pub(crate) struct SnapshotBoxes {
    pub boxes: Vec<(Aabb, u32)>,
    seen: HashSet<[u32; 7]>,
    important: HashSet<[u32; 7]>,
    pub truncated: bool,
}
impl SnapshotBoxes {
    pub fn push(&mut self, bounds: Aabb, depth: u32) {
        if !bounds.min.into_iter().chain(bounds.max).all(f32::is_finite) {
            return;
        }
        let key = box_key(bounds, depth);
        if self.seen.contains(&key) {
            return;
        }
        if self.boxes.len() >= MAX_CAPTURE_NODES {
            self.truncated = true;
            return;
        }
        self.seen.insert(key);
        self.boxes.push((bounds, depth));
    }
    pub fn full(&self) -> bool {
        self.boxes.len() >= MAX_CAPTURE_NODES
    }
    pub fn push_important(&mut self, bounds: Aabb, depth: u32) {
        self.important.insert(box_key(bounds, depth));
        self.push(bounds, depth);
    }
    pub fn limit(&mut self) {
        if self.boxes.len() <= MAX_BOXES {
            return;
        }
        self.truncated = true;
        // Keep outer roots and every damaged cell's mesh root first, then
        // spend the remaining budget on shallow levels across the whole scene.
        self.boxes.sort_by_key(|(bounds, depth)| {
            (!self.important.contains(&box_key(*bounds, *depth)), *depth)
        });
        self.boxes.truncate(MAX_BOXES);
    }
    pub fn mesh(&self) -> CpuMesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut edges = HashSet::<[u32; 7]>::new();
        for (bounds, depth) in &self.boxes {
            let corners: [[f32; 3]; 8] = std::array::from_fn(|i| {
                std::array::from_fn(|axis| {
                    if i & (1 << axis) == 0 {
                        bounds.min[axis]
                    } else {
                        bounds.max[axis]
                    }
                })
            });
            for i in 0..8 {
                for axis in 0..3 {
                    if i & (1 << axis) != 0 {
                        continue;
                    }
                    let a = corners[i];
                    let b = corners[i | (1 << axis)];
                    let length = b[axis] - a[axis];
                    if length < 1e-6 {
                        continue;
                    }
                    let key = [
                        a[0].to_bits(),
                        a[1].to_bits(),
                        a[2].to_bits(),
                        b[0].to_bits(),
                        b[1].to_bits(),
                        b[2].to_bits(),
                        *depth,
                    ];
                    if !edges.insert(key) {
                        continue;
                    }
                    let thickness = 0.004 / (1. + *depth as f32 * 0.08);
                    // Two crossed, double-sided quads replace each 24-vertex
                    // cube rail. This cuts upload vertices by 3x and triangles
                    // by one third while keeping world-space wire thickness.
                    for transverse in [(axis + 1) % 3, (axis + 2) % 3] {
                        let base = vertices.len() as u32;
                        for (end, sign) in [(a, -1.), (a, 1.), (b, 1.), (b, -1.)] {
                            let mut pos = end;
                            pos[transverse] += sign * thickness * 0.5;
                            let mut normal = [0.; 3];
                            normal[3 - axis - transverse] = 1.;
                            vertices.push(CpuVertex {
                                pos,
                                normal,
                                uv: [(sign + 1.) * 0.5, *depth as f32],
                            });
                        }
                        indices.extend([0, 1, 2, 0, 2, 3, 2, 1, 0, 3, 2, 0].map(|i| base + i));
                    }
                }
            }
        }
        CpuMesh::new(vertices, indices)
    }
}
fn box_key(bounds: Aabb, depth: u32) -> [u32; 7] {
    std::array::from_fn(|i| match i {
        0..=2 => bounds.min[i].to_bits(),
        3..=5 => bounds.max[i - 3].to_bits(),
        _ => depth,
    })
}
pub(crate) fn in_scope(world: &World, id: ComponentId, roots: &[ComponentId]) -> bool {
    let mut current = Some(id);
    let mut included = false;
    while let Some(node) = current {
        if world
            .get_component_by_id_as::<EditorUIComponent>(node)
            .is_some()
        {
            return false;
        }
        included |= roots.contains(&node);
        current = world.parent_of(node);
    }
    included
}
/// Preserve ancestor depth and visit parents before children. Detail trees are
/// attached by the leaf callback rather than animated as independent roots.
pub(crate) fn collect<Shape: BHShape>(
    bvh: &BVH,
    shapes: &[Shape],
    wanted: impl Fn(usize) -> bool,
    mut leaf: impl FnMut(usize, u32, &mut SnapshotBoxes),
    out: &mut SnapshotBoxes,
) {
    fn mark(
        bvh: &BVH,
        index: usize,
        wanted: &impl Fn(usize) -> bool,
        included: &mut [bool],
    ) -> bool {
        let keep = match bvh.nodes[index] {
            BVHNode::Leaf { shape_index, .. } => wanted(shape_index),
            BVHNode::Node {
                child_l_index,
                child_r_index,
                ..
            } => {
                let a = mark(bvh, child_l_index, wanted, included);
                let b = mark(bvh, child_r_index, wanted, included);
                a || b
            }
        };
        included[index] = keep;
        keep
    }
    if bvh.nodes.is_empty() {
        return;
    }
    let mut included = vec![false; bvh.nodes.len()];
    mark(bvh, 0, &wanted, &mut included);
    let mut stack = vec![0];
    while let Some(index) = stack.pop() {
        if !included[index] {
            continue;
        }
        if out.full() {
            out.truncated = true;
            break;
        }
        let node = &bvh.nodes[index];
        let a = node.get_node_aabb(shapes);
        let bounds = Aabb {
            min: [a.min.x, a.min.y, a.min.z],
            max: [a.max.x, a.max.y, a.max.z],
        };
        if index == 0 {
            out.push_important(bounds, node.depth());
        } else {
            out.push(bounds, node.depth());
        }
        match *node {
            BVHNode::Leaf { shape_index, .. } => leaf(shape_index, node.depth(), out),
            BVHNode::Node {
                child_l_index,
                child_r_index,
                ..
            } => {
                stack.push(child_r_index);
                stack.push(child_l_index);
            }
        }
    }
}
