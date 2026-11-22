use std::collections::HashMap;

use crate::engine::graphics::mesh::CpuMesh;
use crate::utils::math::{vec3_cross, vec3_len, vec3_scale, vec3_sub};

/// Source triangles with welded connectivity, shared by atlas generation and
/// the occlusion terms.
///
/// glTF splits vertices at normal and UV seams, so adjacency is built on
/// positions welded within `weld_epsilon`. Touching parts of a combined mesh
/// also weld; their contact edges then count as creases.
pub(super) struct Surface {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// Welded vertex id per triangle corner.
    pub welded: Vec<[u32; 3]>,
    pub face_normals: Vec<[f32; 3]>,
    /// Twice the triangle area, i.e. `|cross(b - a, c - a)|`.
    pub double_areas: Vec<f32>,
    pub degenerate: Vec<bool>,
    pub weld_epsilon: f32,
    /// Welded edge `(min, max)` to every triangle using it.
    pub edges: HashMap<(u32, u32), Vec<u32>>,
}

impl Surface {
    pub fn new(mesh: &CpuMesh) -> Self {
        let positions: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| v.pos).collect();
        let triangles: Vec<[u32; 3]> = mesh
            .indices_u32
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]])
            .collect();

        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in &positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let diagonal = if positions.is_empty() {
            0.0
        } else {
            vec3_len(vec3_sub(hi, lo))
        };
        let weld_epsilon = (diagonal * 1e-5).max(1e-7);

        let mut keys: HashMap<[i64; 3], u32> = HashMap::new();
        let weld_ids: Vec<u32> = positions
            .iter()
            .map(|p| {
                let key = p.map(|x| (x / weld_epsilon).round() as i64);
                let next = keys.len() as u32;
                *keys.entry(key).or_insert(next)
            })
            .collect();

        let mut welded = Vec::with_capacity(triangles.len());
        let mut face_normals = Vec::with_capacity(triangles.len());
        let mut double_areas = Vec::with_capacity(triangles.len());
        let mut degenerate = Vec::with_capacity(triangles.len());
        let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
        for (t, tri) in triangles.iter().enumerate() {
            let [a, b, c] = tri.map(|i| positions[i as usize]);
            let cross = vec3_cross(vec3_sub(b, a), vec3_sub(c, a));
            let double_area = vec3_len(cross);
            let is_degenerate = double_area < weld_epsilon * weld_epsilon;
            let w = tri.map(|i| weld_ids[i as usize]);
            welded.push(w);
            face_normals.push(if is_degenerate {
                [0.0, 0.0, 0.0]
            } else {
                vec3_scale(cross, 1.0 / double_area)
            });
            double_areas.push(double_area);
            degenerate.push(is_degenerate);
            if is_degenerate {
                continue;
            }
            for e in 0..3 {
                let (u, v) = (w[e], w[(e + 1) % 3]);
                if u != v {
                    edges.entry((u.min(v), u.max(v))).or_default().push(t as u32);
                }
            }
        }

        Self {
            positions,
            triangles,
            welded,
            face_normals,
            double_areas,
            degenerate,
            weld_epsilon,
            edges,
        }
    }

    pub fn corners(&self, triangle: usize) -> [[f32; 3]; 3] {
        self.triangles[triangle].map(|i| self.positions[i as usize])
    }

    /// Triangles sharing a welded edge with `triangle`.
    pub fn neighbours(&self, triangle: usize, out: &mut Vec<u32>) {
        out.clear();
        let w = self.welded[triangle];
        for e in 0..3 {
            let (u, v) = (w[e], w[(e + 1) % 3]);
            if let Some(list) = self.edges.get(&(u.min(v), u.max(v))) {
                out.extend(list.iter().copied().filter(|&n| n as usize != triangle));
            }
        }
    }
}
