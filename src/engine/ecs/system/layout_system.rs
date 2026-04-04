use crate::engine::ecs::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{RenderableComponent, TextComponent, TransformComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::primitives::{CpuMeshHandle, TransformMatrix};

/// Axis-aligned bounding box in world space.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn empty() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min[0] > self.max[0]
    }

    pub fn width(&self) -> f32 {
        (self.max[0] - self.min[0]).max(0.0)
    }

    pub fn height(&self) -> f32 {
        (self.max[1] - self.min[1]).max(0.0)
    }

    pub fn depth(&self) -> f32 {
        (self.max[2] - self.min[2]).max(0.0)
    }

    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn union(mut self, other: Aabb) -> Self {
        if other.is_empty() { return self; }
        if self.is_empty() { return other; }
        for i in 0..3 {
            self.min[i] = self.min[i].min(other.min[i]);
            self.max[i] = self.max[i].max(other.max[i]);
        }
        self
    }

    pub fn expand_to_point(&mut self, p: [f32; 3]) {
        for i in 0..3 {
            self.min[i] = self.min[i].min(p[i]);
            self.max[i] = self.max[i].max(p[i]);
        }
    }
}

/// Approximate average character width in glyph-local units (pre-transform).
///
/// The text system renders characters at roughly this many units wide in local space.
/// Calibrated against the font used by the panel text rows.
const CHAR_WIDTH_GLYPH: f32 = 0.55;
/// Character height in glyph-local units.
const CHAR_HEIGHT_GLYPH: f32 = 1.0;

/// System that computes layout bounding boxes for ECS subtrees.
///
/// `subtree_aabb` walks a component subtree and unions the world-space AABBs of all
/// `RenderableComponent` and `TextComponent` nodes.
///
/// **Requires:** `TransformComponent.matrix_world` must be up-to-date (i.e., called
/// after `RegisterTransform` intents have been processed, not during initial setup).
#[derive(Debug, Default)]
pub struct LayoutSystem;

impl LayoutSystem {
    pub fn new() -> Self {
        Self
    }

    /// Compute the world-space AABB of all renderable geometry in the subtree rooted at `root`.
    ///
    /// Returns `None` if the subtree contains no renderable geometry.
    pub fn subtree_aabb(world: &World, root: ComponentId) -> Option<Aabb> {
        let mut result = Aabb::empty();
        Self::visit(world, root, &mut result);
        if result.is_empty() { None } else { Some(result) }
    }

    fn visit(world: &World, cid: ComponentId, acc: &mut Aabb) {
        // Renderable — compute world AABB from mesh type and world matrix.
        if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(cid) {
            let mesh = r.renderable.base_mesh;
            if let Some(model) = TransformSystem::world_model(world, cid) {
                if let Some((mn, mx)) = Self::mesh_aabb(mesh, model) {
                    acc.expand_to_point(mn);
                    acc.expand_to_point(mx);
                }
            }
        }

        // Text — estimate AABB from string length × char dimensions × transform scale.
        if let Some(txt) = world.get_component_by_id_as::<TextComponent>(cid) {
            if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid)
                .or_else(|| {
                    // Text nodes are often non-Transform; find nearest ancestor.
                    let mut cur = world.parent_of(cid);
                    loop {
                        let id = cur?;
                        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(id) {
                            return Some(t);
                        }
                        cur = world.parent_of(id);
                    }
                })
            {
                let model = t.transform.matrix_world;
                let line_len = txt.text.lines()
                    .map(|l| l.chars().count())
                    .max()
                    .unwrap_or(0)
                    .min(txt.wrap_at) as f32;
                let num_lines = txt.text.lines().count().max(1) as f32;
                let w = line_len * CHAR_WIDTH_GLYPH;
                let h = num_lines * CHAR_HEIGHT_GLYPH;
                // Text renders in local XY plane starting at origin.
                let corners = [
                    [0.0_f32, 0.0, 0.0],
                    [w, 0.0, 0.0],
                    [0.0, -h, 0.0],
                    [w, -h, 0.0],
                ];
                for p in &corners {
                    let wp = mat4_transform_point(model, *p);
                    acc.expand_to_point(wp);
                }
            }
        }

        for &child in world.children_of(cid) {
            Self::visit(world, child, acc);
        }
    }

    /// World-space AABB for a mesh at the given world transform.
    ///
    /// Shared with `RayCastSystem` — extracted here so layout doesn't depend on raycast.
    pub fn mesh_aabb(mesh: CpuMeshHandle, m: TransformMatrix) -> Option<([f32; 3], [f32; 3])> {
        let (local_pts, thickness): (Vec<[f32; 3]>, f32) = match mesh {
            CpuMeshHandle::CUBE => (
                vec![
                    [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5],
                    [-0.5,  0.5, -0.5], [0.5,  0.5, -0.5],
                    [-0.5, -0.5,  0.5], [0.5, -0.5,  0.5],
                    [-0.5,  0.5,  0.5], [0.5,  0.5,  0.5],
                ],
                0.0,
            ),
            CpuMeshHandle::SPHERE => (
                vec![
                    [-0.5, 0.0, 0.0], [0.5, 0.0, 0.0],
                    [0.0, -0.5, 0.0], [0.0, 0.5, 0.0],
                    [0.0, 0.0, -0.5], [0.0, 0.0, 0.5],
                ],
                0.0,
            ),
            CpuMeshHandle::QUAD_2D
            | CpuMeshHandle::TRIANGLE_2D
            | CpuMeshHandle::CIRCLE_2D => (
                vec![
                    [-0.5, -0.5, 0.0], [0.5, -0.5, 0.0],
                    [-0.5,  0.5, 0.0], [0.5,  0.5, 0.0],
                ],
                0.01,
            ),
            _ => return None,
        };

        let mut mn = [f32::INFINITY; 3];
        let mut mx = [f32::NEG_INFINITY; 3];
        for p in &local_pts {
            let w = mat4_transform_point(m, *p);
            for i in 0..3 {
                mn[i] = mn[i].min(w[i]);
                mx[i] = mx[i].max(w[i]);
            }
        }
        if thickness > 0.0 {
            mn[2] -= thickness;
            mx[2] += thickness;
        }
        Some((mn, mx))
    }

    /// Estimate the overlay-space width of a text panel from known constants, without needing
    /// world matrices. Used during panel setup before transforms are propagated.
    ///
    /// - `max_chars`: estimated max label length per row (e.g. `wrap_at`)
    /// - `text_scale`: the `TransformComponent` scale applied to text rows
    /// - `indent_width`: total x indent for the deepest row (e.g. `MAX_DEPTH * INDENT_UNIT`)
    pub fn estimate_panel_width(max_chars: usize, text_scale: f32, indent_width: f32) -> f32 {
        indent_width + max_chars as f32 * CHAR_WIDTH_GLYPH * text_scale
    }
}

fn mat4_transform_point(m: TransformMatrix, p: [f32; 3]) -> [f32; 3] {
    let v = [p[0], p[1], p[2], 1.0_f32];
    let w = [
        m[0][0]*v[0] + m[1][0]*v[1] + m[2][0]*v[2] + m[3][0]*v[3],
        m[0][1]*v[0] + m[1][1]*v[1] + m[2][1]*v[2] + m[3][1]*v[3],
        m[0][2]*v[0] + m[1][2]*v[1] + m[2][2]*v[2] + m[3][2]*v[3],
    ];
    w
}
