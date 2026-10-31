//! Snapshot-only BVH extraction and batched wire geometry. Never called to
//! maintain the diagnostic between explicit editor requests.
use crate::engine::ecs::component::EditorUIComponent;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    bounds::Aabb,
    mesh::{CpuMesh, CpuVertex, MeshFactory},
};
use bvh::{
    bounding_hierarchy::BHShape,
    bvh::{BVH, BVHNode},
};
use std::collections::HashSet;
pub const MAX_BOXES: usize = 4096;
#[derive(Debug, Default)]
pub(crate) struct SnapshotBoxes {
    pub boxes: Vec<(Aabb, u32)>,
    seen: HashSet<[u32; 6]>,
    pub truncated: bool,
}
impl SnapshotBoxes {
    pub fn push(&mut self, bounds: Aabb, depth: u32) {
        if !bounds.min.into_iter().chain(bounds.max).all(f32::is_finite) {
            return;
        }
        let key = std::array::from_fn(|i| {
            if i < 3 {
                bounds.min[i].to_bits()
            } else {
                bounds.max[i - 3].to_bits()
            }
        });
        if self.seen.contains(&key) {
            return;
        }
        if self.boxes.len() >= MAX_BOXES {
            self.truncated = true;
            return;
        }
        self.seen.insert(key);
        self.boxes.push((bounds, depth));
    }
    pub fn full(&self) -> bool {
        self.boxes.len() >= MAX_BOXES
    }
    pub fn mesh(&self) -> CpuMesh {
        let cube = MeshFactory::cube();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut edges = HashSet::<[u32; 6]>::new();
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
                    ];
                    if !edges.insert(key) {
                        continue;
                    }
                    let thickness = 0.004 / (1. + *depth as f32 * 0.08);
                    let center: [f32; 3] = std::array::from_fn(|j| (a[j] + b[j]) * 0.5);
                    let mut size = [thickness; 3];
                    size[axis] = length + thickness;
                    let base = vertices.len() as u32;
                    vertices.extend(cube.vertices.iter().map(|v| CpuVertex {
                        pos: std::array::from_fn(|j| center[j] + v.pos[j] * size[j]),
                        normal: v.normal,
                        uv: v.uv,
                    }));
                    indices.extend(cube.indices_u32.iter().map(|i| base + i));
                }
            }
        }
        CpuMesh::new(vertices, indices)
    }
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
pub(crate) fn collect<Shape: BHShape>(
    bvh: &BVH,
    shapes: &[Shape],
    wanted: impl Fn(usize) -> bool,
    out: &mut SnapshotBoxes,
) {
    fn visit<S: BHShape>(
        bvh: &BVH,
        shapes: &[S],
        index: usize,
        wanted: &impl Fn(usize) -> bool,
        out: &mut SnapshotBoxes,
    ) -> bool {
        if out.full() {
            out.truncated = true;
            return false;
        }
        let node = &bvh.nodes[index];
        let include = match *node {
            BVHNode::Leaf { shape_index, .. } => wanted(shape_index),
            BVHNode::Node {
                child_l_index,
                child_r_index,
                ..
            } => {
                let a = visit(bvh, shapes, child_l_index, wanted, out);
                let b = visit(bvh, shapes, child_r_index, wanted, out);
                a || b
            }
        };
        if include {
            let a = node.get_node_aabb(shapes);
            out.push(
                Aabb {
                    min: [a.min.x, a.min.y, a.min.z],
                    max: [a.max.x, a.max.y, a.max.z],
                },
                node.depth(),
            );
        }
        include
    }
    if !bvh.nodes.is_empty() {
        visit(bvh, shapes, 0, &wanted, out);
    }
}
