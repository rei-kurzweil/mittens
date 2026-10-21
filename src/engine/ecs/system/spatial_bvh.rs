//! Shared BVH maintenance and conservative AABB traversal, independent of leaf semantics.
use bvh::aabb::AABB;
use bvh::bounding_hierarchy::BHShape;
use bvh::bvh::{BVH, BVHNode};
use std::collections::HashSet;

/// Refit without rotating nodes. bvh 0.7's optimizer can panic when a queued
/// ancestor changes depth during optimization of several moving leaves.
pub(crate) fn refit<Shape: BHShape>(bvh: &mut BVH, shapes: &[Shape], changed: &HashSet<usize>) {
    let mut ancestors = HashSet::new();
    for &shape_index in changed {
        let mut node = shapes[shape_index].bh_node_index();
        while node != 0 {
            node = bvh.nodes[node].parent();
            ancestors.insert(node);
        }
    }
    let mut ancestors: Vec<_> = ancestors.into_iter().collect();
    ancestors.sort_unstable_by_key(|&index| std::cmp::Reverse(bvh.nodes[index].depth()));
    for index in ancestors {
        let BVHNode::Node {
            child_l_index,
            child_r_index,
            ..
        } = bvh.nodes[index]
        else {
            continue;
        };
        let left = bvh.nodes[child_l_index].get_node_aabb(shapes);
        let right = bvh.nodes[child_r_index].get_node_aabb(shapes);
        *bvh.nodes[index].child_l_aabb_mut() = left;
        *bvh.nodes[index].child_r_aabb_mut() = right;
    }
}

pub(crate) fn overlaps(a: &AABB, b: &AABB) -> bool {
    a.min.x <= b.max.x
        && b.min.x <= a.max.x
        && a.min.y <= b.max.y
        && b.min.y <= a.max.y
        && a.min.z <= b.max.z
        && b.min.z <= a.max.z
}

/// Returns visited node count. Caller owns reusable traversal/result storage.
pub(crate) fn query<Shape: BHShape>(
    bvh: &BVH,
    shapes: &[Shape],
    bounds: &AABB,
    stack: &mut Vec<usize>,
    out: &mut Vec<usize>,
) -> u64 {
    stack.clear();
    out.clear();
    if bvh.nodes.is_empty() {
        return 0;
    }
    stack.push(0);
    let mut visits = 0;
    while let Some(index) = stack.pop() {
        visits += 1;
        match bvh.nodes[index] {
            BVHNode::Node {
                child_l_index,
                child_l_aabb,
                child_r_index,
                child_r_aabb,
                ..
            } => {
                if overlaps(bounds, &child_l_aabb) {
                    stack.push(child_l_index);
                }
                if overlaps(bounds, &child_r_aabb) {
                    stack.push(child_r_index);
                }
            }
            BVHNode::Leaf { shape_index, .. } => {
                if overlaps(bounds, &shapes[shape_index].aabb()) {
                    out.push(shape_index);
                }
            }
        }
    }
    visits
}
