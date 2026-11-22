use std::collections::HashMap;

use super::AoBakeSettings;
use super::surface::Surface;
use crate::utils::math::{vec3_add, vec3_dot, vec3_len, vec3_scale, vec3_sub};

/// Fraction of the proximity radius within which a parallel, facing layer
/// means "this side is inside the material" (solidified panels).
const THICKNESS_FRACTION: f32 = 0.4;
const FACING: f32 = 0.7;
/// Creases ramp from `cavity_angle_deg` to full strength over this angle.
const CREASE_RAMP_DEG: f32 = 30.0;

/// Occlusion seen from one texel, after choosing which side of its surface
/// it represents.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Occlusion {
    pub proximity: f32,
    pub cavity: f32,
}

struct Crease {
    a: [f32; 3],
    b: [f32; 3],
    /// Points into the side the crease is concave on.
    concave_side: [f32; 3],
    strength: f32,
    radius: f32,
}

/// Uniform grid whose entries are inserted into every cell their influence
/// box touches, so a query only reads the query point's own cell.
struct Grid {
    cell: f32,
    cells: HashMap<[i32; 3], Vec<u32>>,
}

impl Grid {
    fn new(cell: f32) -> Self {
        Self {
            cell,
            cells: HashMap::new(),
        }
    }

    fn key(&self, p: [f32; 3]) -> [i32; 3] {
        p.map(|x| (x / self.cell).floor() as i32)
    }

    fn insert(&mut self, id: u32, min: [f32; 3], max: [f32; 3]) {
        let (lo, hi) = (self.key(min), self.key(max));
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    self.cells.entry([x, y, z]).or_default().push(id);
                }
            }
        }
    }

    fn query(&self, p: [f32; 3]) -> &[u32] {
        self.cells.get(&self.key(p)).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn bounds(points: &[[f32; 3]], pad: f32) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in points {
        for k in 0..3 {
            min[k] = min[k].min(p[k] - pad);
            max[k] = max[k].max(p[k] + pad);
        }
    }
    (min, max)
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Concave edges sharper than the threshold, on whichever side they are
/// concave. A crease darkens about as far as its walls extend, and point-like
/// (very short) creases are weakened so they do not paint round blobs.
fn creases(surface: &Surface, settings: &AoBakeSettings) -> Vec<Crease> {
    let threshold = settings.cavity_angle_deg.to_radians();
    let ramp = CREASE_RAMP_DEG.to_radians();
    let mut out = Vec::new();
    for (&(u, v), triangles) in &surface.edges {
        let &[ta, tb] = triangles.as_slice() else {
            continue;
        };
        let (ta, tb) = (ta as usize, tb as usize);
        let (na, nb) = (surface.face_normals[ta], surface.face_normals[tb]);
        let angle = vec3_dot(na, nb).clamp(-1.0, 1.0).acos();
        let mut strength = smoothstep((angle - threshold) / ramp);
        if strength <= 0.0 {
            continue;
        }
        let Some(opposite) = (0..3).find(|&k| {
            let w = surface.welded[tb][k];
            w != u && w != v
        }) else {
            continue;
        };
        let a0 = surface.positions[surface.triangles[ta][0] as usize];
        let opposite = surface.positions[surface.triangles[tb][opposite] as usize];
        let height = vec3_dot(na, vec3_sub(opposite, a0));
        if height.abs() <= surface.weld_epsilon {
            continue;
        }
        let endpoint = |welded: u32| {
            (0..3)
                .find(|&k| surface.welded[ta][k] == welded)
                .map(|k| surface.positions[surface.triangles[ta][k] as usize])
        };
        let (Some(a), Some(b)) = (endpoint(u), endpoint(v)) else {
            continue;
        };
        let length = vec3_len(vec3_sub(b, a));
        if length <= surface.weld_epsilon {
            continue;
        }
        let walls = surface.double_areas[ta].min(surface.double_areas[tb]) / length;
        let radius = settings.cavity_radius.min(walls);
        if radius <= 0.0 {
            continue;
        }
        strength *= (length / radius).min(1.0);
        let sign = if height > 0.0 { 1.0 } else { -1.0 };
        out.push(Crease {
            a,
            b,
            concave_side: vec3_scale(vec3_add(na, nb), sign),
            strength,
            radius,
        });
    }
    out
}

fn closest_on_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    let d = vec3_sub(b, a);
    let t = (vec3_dot(vec3_sub(p, a), d) / vec3_dot(d, d).max(1e-30)).clamp(0.0, 1.0);
    vec3_add(a, vec3_scale(d, t))
}

/// Ericson, Real-Time Collision Detection 5.1.5.
fn closest_on_triangle(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = vec3_sub(b, a);
    let ac = vec3_sub(c, a);
    let ap = vec3_sub(p, a);
    let (d1, d2) = (vec3_dot(ab, ap), vec3_dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = vec3_sub(p, b);
    let (d3, d4) = (vec3_dot(ab, bp), vec3_dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return vec3_add(a, vec3_scale(ab, d1 / (d1 - d3)));
    }
    let cp = vec3_sub(p, c);
    let (d5, d6) = (vec3_dot(ab, cp), vec3_dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return vec3_add(a, vec3_scale(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return vec3_add(b, vec3_scale(vec3_sub(c, b), w));
    }
    let denom = 1.0 / (va + vb + vc);
    vec3_add(a, vec3_add(vec3_scale(ab, vb * denom), vec3_scale(ac, vc * denom)))
}

/// Evaluates proximity and cavity for each `(triangle, position)` sample.
///
/// One-sided bakes use the winding side. Two-sided bakes measure both sides
/// of the sample's surface, because the renderer shades both faces of a
/// single sheet from the same texel: a side with a parallel layer right
/// behind it is inside solid material and is ignored; otherwise the more
/// open side wins.
pub(super) fn evaluate(
    surface: &Surface,
    samples: &[(u32, [f32; 3])],
    settings: &AoBakeSettings,
    texels_per_unit: f32,
) -> Vec<Occlusion> {
    let proximity_radius = settings.proximity_radius.max(1e-6);
    let thickness = proximity_radius * THICKNESS_FRACTION;
    // Exactly coincident surfaces are duplicates, not occluders.
    let min_distance = 0.25 / texels_per_unit.max(1e-6);

    let mut triangles = Grid::new(proximity_radius);
    for t in 0..surface.triangles.len() {
        if !surface.degenerate[t] {
            let (min, max) = bounds(&surface.corners(t), proximity_radius);
            triangles.insert(t as u32, min, max);
        }
    }
    let creases = creases(surface, settings);
    let mut crease_grid = Grid::new(settings.cavity_radius.max(1e-6));
    for (i, crease) in creases.iter().enumerate() {
        let (min, max) = bounds(&[crease.a, crease.b], crease.radius);
        crease_grid.insert(i as u32, min, max);
    }

    samples
        .iter()
        .map(|&(own, p)| {
            let normal = surface.face_normals[own as usize];
            let mut proximity = [0.0f32; 2];
            let mut cavity = [0.0f32; 2];
            let mut inside = [false; 2];

            for &t in triangles.query(p) {
                if t == own {
                    continue;
                }
                let [a, b, c] = surface.corners(t as usize);
                let offset = vec3_sub(closest_on_triangle(p, a, b, c), p);
                let distance = vec3_len(offset);
                if distance < 1e-9 {
                    continue;
                }
                let facing = vec3_dot(normal, offset) / distance;
                let side = usize::from(facing <= 0.0);
                if distance < thickness
                    && facing.abs() > FACING
                    && vec3_dot(surface.face_normals[t as usize], normal).abs() > FACING
                {
                    inside[side] = true;
                }
                if distance <= min_distance || distance >= proximity_radius {
                    continue;
                }
                let falloff = 1.0 - distance / proximity_radius;
                proximity[side] = proximity[side].max(falloff * falloff * facing.abs());
            }

            for &i in crease_grid.query(p) {
                let crease = &creases[i as usize];
                let distance = vec3_len(vec3_sub(p, closest_on_segment(p, crease.a, crease.b)));
                if distance >= crease.radius {
                    continue;
                }
                let falloff = 1.0 - distance / crease.radius;
                let side = usize::from(vec3_dot(normal, crease.concave_side) <= 0.0);
                cavity[side] = cavity[side].max(crease.strength * falloff * falloff);
            }

            let total = [proximity[0].max(cavity[0]), proximity[1].max(cavity[1])];
            let side = match (inside[0], inside[1]) {
                _ if !settings.two_sided => 0,
                (true, false) => 1,
                (false, true) => 0,
                _ if total[1] < total[0] => 1,
                _ => 0,
            };
            Occlusion {
                proximity: proximity[side],
                cavity: cavity[side],
            }
        })
        .collect()
}
