use std::collections::HashMap;
use std::fmt;

use super::surface::Surface;
use crate::engine::graphics::mesh::CpuMesh;
use crate::utils::math::{vec3_cross, vec3_dot, vec3_normalize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvGenOptions {
    /// Square atlas size in texels.
    pub resolution: u32,
    /// Gutter in texels around every chart.
    pub padding: u32,
}

/// A unique, non-overlapping UV layout for a mesh.
///
/// Triangle `t` of `indices` is source triangle `t`. Output vertex `i` copies
/// source vertex `xref[i]`; seams duplicate vertices, so skinning or morph
/// data is remapped through `xref` too.
#[derive(Debug, Clone)]
pub struct UvAtlas {
    pub xref: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub triangle_charts: Vec<u32>,
    pub chart_count: u32,
    pub width: u32,
    pub height: u32,
    /// Atlas texels per mesh-space unit.
    pub texels_per_unit: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UvGenError {
    NoTriangles,
    /// Every chart needs at least one texel plus its gutters.
    DoesNotFit { charts: usize, resolution: u32 },
}

impl fmt::Display for UvGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTriangles => write!(f, "mesh has no triangles"),
            Self::DoesNotFit { charts, resolution } => write!(
                f,
                "{charts} charts do not fit a {resolution}x{resolution} atlas; raise the resolution"
            ),
        }
    }
}

impl std::error::Error for UvGenError {}

pub trait UvGenerator {
    fn generate(&self, mesh: &CpuMesh, options: &UvGenOptions) -> Result<UvAtlas, UvGenError>;
}

/// Pure-Rust atlas: normal-cone charts with an overlap guard, planar
/// projection, minimum-area rotation, and shelf packing.
///
/// `xatlas` is the intended default once its binding exists; this generator
/// stays as the dependency-free path.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinUvGenerator {
    /// Largest angle between a chart's seed normal and any member normal.
    pub max_chart_angle_deg: f32,
    /// Bounds the quadratic overlap check per chart.
    pub max_chart_triangles: usize,
}

impl Default for BuiltinUvGenerator {
    fn default() -> Self {
        Self {
            max_chart_angle_deg: 55.0,
            max_chart_triangles: 1024,
        }
    }
}

struct Chart {
    triangles: Vec<u32>,
    /// Source vertex to chart-local position, origin at the chart's min corner.
    local: HashMap<u32, [f32; 2]>,
    extent: [f32; 2],
}

impl UvGenerator for BuiltinUvGenerator {
    fn generate(&self, mesh: &CpuMesh, options: &UvGenOptions) -> Result<UvAtlas, UvGenError> {
        let surface = Surface::new(mesh);
        if surface.triangles.is_empty() {
            return Err(UvGenError::NoTriangles);
        }
        let charts = self.build_charts(&surface);
        pack(&surface, charts, options)
    }
}

impl BuiltinUvGenerator {
    fn build_charts(&self, surface: &Surface) -> Vec<Chart> {
        let cos_max = self.max_chart_angle_deg.to_radians().cos();
        let count = surface.triangles.len();
        let mut assigned = vec![false; count];
        let mut order: Vec<usize> = (0..count).filter(|&t| !surface.degenerate[t]).collect();
        order.sort_by(|&a, &b| {
            surface.double_areas[b]
                .total_cmp(&surface.double_areas[a])
                .then(a.cmp(&b))
        });

        let mut charts = Vec::new();
        let mut neighbours = Vec::new();
        let mut stack = Vec::new();
        for seed in order {
            if assigned[seed] {
                continue;
            }
            let seed_normal = surface.face_normals[seed];
            let (tu, tv) = tangent_basis(seed_normal);
            let project = |t: usize| surface.corners(t).map(|p| [vec3_dot(p, tu), vec3_dot(p, tv)]);
            assigned[seed] = true;
            let mut members = vec![seed as u32];
            let mut placed = vec![project(seed)];
            stack.clear();
            stack.push(seed);
            while let Some(t) = stack.pop() {
                surface.neighbours(t, &mut neighbours);
                for &n in &neighbours {
                    let n = n as usize;
                    if assigned[n]
                        || surface.degenerate[n]
                        || members.len() >= self.max_chart_triangles
                        || vec3_dot(surface.face_normals[n], seed_normal) < cos_max
                    {
                        continue;
                    }
                    let flat = project(n);
                    if placed.iter().any(|other| overlaps_2d(&flat, other)) {
                        continue;
                    }
                    assigned[n] = true;
                    members.push(n as u32);
                    placed.push(flat);
                    stack.push(n);
                }
            }
            charts.push(layout_chart(surface, members, tu, tv));
        }

        // Degenerate triangles carry no texels; collapse them onto the first
        // chart's origin so every triangle still has UVs.
        let degenerate: Vec<u32> = (0..count as u32)
            .filter(|&t| surface.degenerate[t as usize])
            .collect();
        if !degenerate.is_empty() {
            if charts.is_empty() {
                charts.push(Chart {
                    triangles: Vec::new(),
                    local: HashMap::new(),
                    extent: [0.0, 0.0],
                });
            }
            for t in degenerate {
                for v in surface.triangles[t as usize] {
                    charts[0].local.entry(v).or_insert([0.0, 0.0]);
                }
                charts[0].triangles.push(t);
            }
        }
        charts
    }
}

fn tangent_basis(normal: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let up = if normal[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let tu = vec3_normalize(vec3_cross(up, normal));
    let tv = vec3_cross(normal, tu);
    (tu, tv)
}

/// Separating-axis test on the six edge normals. Shared edges and vertices
/// touch without overlapping.
fn overlaps_2d(a: &[[f32; 2]; 3], b: &[[f32; 2]; 3]) -> bool {
    for tri in [a, b] {
        for e in 0..3 {
            let d = [tri[(e + 1) % 3][0] - tri[e][0], tri[(e + 1) % 3][1] - tri[e][1]];
            let axis = [-d[1], d[0]];
            let tolerance = 1e-6 * (axis[0].abs() + axis[1].abs());
            let range = |t: &[[f32; 2]; 3]| {
                let p = t.map(|v| v[0] * axis[0] + v[1] * axis[1]);
                (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]))
            };
            let (a_min, a_max) = range(a);
            let (b_min, b_max) = range(b);
            if a_max <= b_min + tolerance || b_max <= a_min + tolerance {
                return false;
            }
        }
    }
    true
}

/// Projects a chart, rotates it to the smallest bounding box over its edge
/// directions, and moves its min corner to the origin.
fn layout_chart(surface: &Surface, triangles: Vec<u32>, tu: [f32; 3], tv: [f32; 3]) -> Chart {
    let mut local: HashMap<u32, [f32; 2]> = HashMap::new();
    for &t in &triangles {
        for v in surface.triangles[t as usize] {
            local.entry(v).or_insert_with(|| {
                let p = surface.positions[v as usize];
                [vec3_dot(p, tu), vec3_dot(p, tv)]
            });
        }
    }

    let mut best = (f32::INFINITY, 0.0f32);
    for &t in triangles.iter().take(64) {
        let tri = surface.triangles[t as usize];
        for e in 0..3 {
            let a = local[&tri[e]];
            let b = local[&tri[(e + 1) % 3]];
            let d = [b[0] - a[0], b[1] - a[1]];
            if d[0] * d[0] + d[1] * d[1] < 1e-20 {
                continue;
            }
            let angle = d[1].atan2(d[0]);
            let area = rotated_extent(local.values(), angle);
            if area[0] * area[1] < best.0 {
                best = (area[0] * area[1], angle);
            }
        }
    }

    let (sin, cos) = (-best.1).sin_cos();
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for p in local.values_mut() {
        *p = [p[0] * cos - p[1] * sin, p[0] * sin + p[1] * cos];
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    for p in local.values_mut() {
        *p = [p[0] - min[0], p[1] - min[1]];
    }
    Chart {
        triangles,
        local,
        extent: [max[0] - min[0], max[1] - min[1]],
    }
}

fn rotated_extent<'a>(points: impl Iterator<Item = &'a [f32; 2]>, angle: f32) -> [f32; 2] {
    let (sin, cos) = (-angle).sin_cos();
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for p in points {
        let r = [p[0] * cos - p[1] * sin, p[0] * sin + p[1] * cos];
        for k in 0..2 {
            min[k] = min[k].min(r[k]);
            max[k] = max[k].max(r[k]);
        }
    }
    [max[0] - min[0], max[1] - min[1]]
}

/// Shelf placement of chart rectangles at `scale` texels per unit, tallest
/// first. Returns each chart's texel origin, or `None` if it overflows.
fn shelf_pack(charts: &[Chart], scale: f32, resolution: u32, padding: u32) -> Option<Vec<[u32; 2]>> {
    let size = |c: &Chart| {
        [
            (c.extent[0] * scale).ceil().max(1.0) as u64 + 2 * padding as u64,
            (c.extent[1] * scale).ceil().max(1.0) as u64 + 2 * padding as u64,
        ]
    };
    let mut order: Vec<usize> = (0..charts.len()).collect();
    order.sort_by(|&a, &b| charts[b].extent[1].total_cmp(&charts[a].extent[1]).then(a.cmp(&b)));
    let limit = resolution as u64;
    let (mut x, mut y, mut shelf) = (0u64, 0u64, 0u64);
    let mut origins = vec![[0u32; 2]; charts.len()];
    for c in order {
        let [w, h] = size(&charts[c]);
        if w > limit {
            return None;
        }
        if x + w > limit {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        if y + h > limit {
            return None;
        }
        origins[c] = [x as u32, y as u32];
        x += w;
        shelf = shelf.max(h);
    }
    Some(origins)
}

fn pack(surface: &Surface, charts: Vec<Chart>, options: &UvGenOptions) -> Result<UvAtlas, UvGenError> {
    let resolution = options.resolution;
    let padding = options.padding;
    // Largest scale that still packs, by geometric bisection.
    let (mut lo, mut hi) = (1e-6f32, 1e7f32);
    if shelf_pack(&charts, lo, resolution, padding).is_none() {
        return Err(UvGenError::DoesNotFit {
            charts: charts.len(),
            resolution,
        });
    }
    for _ in 0..48 {
        let mid = (lo * hi).sqrt();
        if shelf_pack(&charts, mid, resolution, padding).is_some() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let scale = lo;
    let origins = shelf_pack(&charts, scale, resolution, padding).expect("scale was verified");

    let triangle_count = surface.triangles.len();
    let mut triangle_charts = vec![0u32; triangle_count];
    for (c, chart) in charts.iter().enumerate() {
        for &t in &chart.triangles {
            triangle_charts[t as usize] = c as u32;
        }
    }

    let inv = 1.0 / resolution as f32;
    let mut remap: HashMap<(u32, u32), u32> = HashMap::new();
    let mut xref = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::with_capacity(triangle_count * 3);
    for t in 0..triangle_count {
        let c = triangle_charts[t];
        let chart = &charts[c as usize];
        let origin = origins[c as usize];
        for v in surface.triangles[t] {
            let index = *remap.entry((c, v)).or_insert_with(|| {
                let p = chart.local.get(&v).copied().unwrap_or([0.0, 0.0]);
                xref.push(v);
                uvs.push([
                    (origin[0] as f32 + padding as f32 + p[0] * scale) * inv,
                    (origin[1] as f32 + padding as f32 + p[1] * scale) * inv,
                ]);
                (xref.len() - 1) as u32
            });
            indices.push(index);
        }
    }

    Ok(UvAtlas {
        xref,
        uvs,
        indices,
        triangle_charts,
        chart_count: charts.len() as u32,
        width: resolution,
        height: resolution,
        texels_per_unit: scale,
    })
}
