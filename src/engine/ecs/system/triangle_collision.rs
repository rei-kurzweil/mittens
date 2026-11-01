//! Retained BVH for the small damaged portion of a static surface. Primitive
//! zones elsewhere keep their existing narrow phase and allocation behavior.
use super::zone_query::ZoneSweepHit;
use crate::engine::ecs::component::{CollisionShape, CutGeometryComponent};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{bounds::Aabb, mesh::CpuMesh, primitives::TransformMatrix};
use crate::utils::math::{
    mat4_mul_vec4, vec3_add as add, vec3_cross as cross, vec3_dot as dot, vec3_normalize as unit,
    vec3_scale as scale, vec3_sub as sub,
};

#[derive(Debug)]
pub struct TriangleMesh {
    triangles: Vec<[[f32; 3]; 3]>,
    nodes: Vec<Node>,
    pub bounds: Aabb,
    cache: std::sync::Mutex<Option<(TransformMatrix, std::sync::Arc<TriangleMesh>)>>,
}
#[derive(Debug)]
struct Node {
    bounds: Aabb,
    start: usize,
    end: usize,
    children: Option<[usize; 2]>,
}
impl TriangleMesh {
    pub(crate) fn snapshot(
        &self,
        out: &mut super::bvh_snapshot_mesh::SnapshotBoxes,
        base_depth: u32,
    ) {
        fn visit(
            mesh: &TriangleMesh,
            index: usize,
            depth: u32,
            out: &mut super::bvh_snapshot_mesh::SnapshotBoxes,
        ) {
            if out.full() {
                out.truncated = true;
                return;
            }
            let node = &mesh.nodes[index];
            out.push(node.bounds, depth);
            if let Some([a, b]) = node.children {
                visit(mesh, a, depth + 1, out);
                visit(mesh, b, depth + 1, out);
            }
        }
        if !self.nodes.is_empty() {
            out.push_important(self.nodes[0].bounds, base_depth);
            visit(self, 0, base_depth, out);
        }
    }
    pub fn new(mesh: &CpuMesh) -> Self {
        let triangles: Vec<_> = mesh
            .indices_u32
            .chunks_exact(3)
            .map(|f| std::array::from_fn(|i| mesh.vertices[f[i] as usize].pos))
            .collect();
        let bounds = Aabb::from_points(&triangles.iter().flatten().copied().collect::<Vec<_>>())
            .unwrap_or(Aabb {
                min: [0.; 3],
                max: [0.; 3],
            });
        let mut result = Self {
            triangles,
            nodes: vec![],
            bounds,
            cache: std::sync::Mutex::new(None),
        };
        if !result.triangles.is_empty() {
            result.build(0, result.triangles.len());
        }
        result
    }
    pub fn world(&self, frame: TransformMatrix) -> std::sync::Arc<Self> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((previous, mesh)) = &*cache {
            if *previous == frame {
                return mesh.clone();
            }
        }
        let mesh = std::sync::Arc::new(self.transformed(frame));
        *cache = Some((frame, mesh.clone()));
        mesh
    }
    pub fn contains(&self, point: [f32; 3]) -> bool {
        let direction = [0.819231, 0.317823, 0.477193];
        let mut crossings = 0;
        self.query(self.bounds, |tri| {
            if ray_triangle(point, direction, tri).is_some_and(|t| t > 1e-6) {
                crossings += 1;
            }
        });
        crossings % 2 == 1
    }
    pub fn transformed(&self, frame: TransformMatrix) -> Self {
        let mesh = CpuMesh::new(
            self.triangles
                .iter()
                .flatten()
                .map(|p| {
                    let q = mat4_mul_vec4(frame, [p[0], p[1], p[2], 1.]);
                    crate::engine::graphics::mesh::CpuVertex {
                        pos: [q[0], q[1], q[2]],
                        uv: [0.; 2],
                        normal: [0.; 3],
                    }
                })
                .collect(),
            (0..self.triangles.len() as u32 * 3).collect(),
        );
        Self::new(&mesh)
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let bounds = Aabb::from_points(
            &self.triangles[start..end]
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            children: None,
        });
        if end - start > 8 {
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (bounds.max[a] - bounds.min[a]).total_cmp(&(bounds.max[b] - bounds.min[b]))
                })
                .unwrap();
            let middle = (start + end) / 2;
            self.triangles[start..end].select_nth_unstable_by(middle - start, |a, b| {
                a.iter()
                    .map(|p| p[axis])
                    .sum::<f32>()
                    .total_cmp(&b.iter().map(|p| p[axis]).sum::<f32>())
            });
            let left = self.build(start, middle);
            let right = self.build(middle, end);
            self.nodes[index].children = Some([left, right]);
        }
        index
    }
    fn query(&self, bounds: Aabb, mut visit: impl FnMut([[f32; 3]; 3])) {
        fn walk(
            mesh: &TriangleMesh,
            index: usize,
            bounds: Aabb,
            visit: &mut impl FnMut([[f32; 3]; 3]),
        ) {
            let node = &mesh.nodes[index];
            if !(0..3)
                .all(|i| bounds.min[i] <= node.bounds.max[i] && bounds.max[i] >= node.bounds.min[i])
            {
                return;
            }
            if let Some([a, b]) = node.children {
                walk(mesh, a, bounds, visit);
                walk(mesh, b, bounds, visit);
            } else {
                for tri in &mesh.triangles[node.start..node.end] {
                    visit(*tri);
                }
            }
        }
        if !self.nodes.is_empty() {
            walk(self, 0, bounds, &mut visit);
        }
    }
    pub fn ray(&self, origin: [f32; 3], dir: [f32; 3]) -> Option<f32> {
        fn walk(mesh: &TriangleMesh, index: usize, o: [f32; 3], d: [f32; 3], best: &mut f32) {
            let node = &mesh.nodes[index];
            let mut near = 0f32;
            let mut far = *best;
            for i in 0..3 {
                if d[i].abs() < 1e-12 {
                    if o[i] < node.bounds.min[i] || o[i] > node.bounds.max[i] {
                        return;
                    }
                } else {
                    let a = (node.bounds.min[i] - o[i]) / d[i];
                    let b = (node.bounds.max[i] - o[i]) / d[i];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                }
            }
            if far < near {
                return;
            }
            if let Some([a, b]) = node.children {
                walk(mesh, a, o, d, best);
                walk(mesh, b, o, d, best);
            } else {
                for tri in &mesh.triangles[node.start..node.end] {
                    if let Some(t) = ray_triangle(o, d, *tri) {
                        *best = best.min(t);
                    }
                }
            }
        }
        let mut best = f32::INFINITY;
        if !self.nodes.is_empty() {
            walk(self, 0, origin, dir, &mut best);
        }
        best.is_finite().then_some(best)
    }
    pub fn translation(&self, center: [f32; 3], shape: CollisionShape) -> Option<[f32; 3]> {
        let (lo, hi) = shape.aabb_local();
        let bounds = Aabb {
            min: add(center, lo),
            max: add(center, hi),
        };
        let mut best = None;
        let mut deepest = 0.;
        self.query(bounds, |tri| {
            let delta = match shape {
                CollisionShape::Cube { half_extents } => box_triangle(center, half_extents, tri),
                _ => {
                    let (radius, half) = capsule(shape);
                    let (a, b) = segment_triangle(
                        sub(center, [0., half, 0.]),
                        add(center, [0., half, 0.]),
                        tri,
                    );
                    let diff = sub(a, b);
                    let distance = dot(diff, diff).sqrt();
                    (distance < radius).then(|| {
                        scale(
                            if distance > 1e-7 {
                                scale(diff, 1. / distance)
                            } else {
                                unit(cross(sub(tri[1], tri[0]), sub(tri[2], tri[0])))
                            },
                            radius - distance,
                        )
                    })
                }
            };
            if let Some(d) = delta {
                let depth = dot(d, d);
                if depth > deepest {
                    deepest = depth;
                    best = Some(d);
                }
            }
        });
        best
    }
    pub fn sweep(
        &self,
        moving: ComponentId,
        surface: ComponentId,
        end: [f32; 3],
        shape: CollisionShape,
        start: [f32; 3],
    ) -> Option<ZoneSweepHit> {
        let motion = sub(end, start);
        let speed = dot(motion, motion).sqrt();
        if speed < 1e-7 {
            return None;
        }
        let (radius, half) = capsule(shape);
        if radius <= 0. {
            return None;
        }
        let (lo, hi) = shape.aabb_local();
        let bounds = Aabb {
            min: std::array::from_fn(|i| start[i].min(end[i]) + lo[i] - 1e-5),
            max: std::array::from_fn(|i| start[i].max(end[i]) + hi[i] + 1e-5),
        };
        let mut result: Option<ZoneSweepHit> = None;
        self.query(bounds, |tri| {
            let mut t = 0.;
            for _ in 0..48 {
                let center = add(start, scale(motion, t));
                let (a, b) = segment_triangle(
                    sub(center, [0., half, 0.]),
                    add(center, [0., half, 0.]),
                    tri,
                );
                let diff = sub(a, b);
                let distance = dot(diff, diff).sqrt();
                let gap = distance - radius;
                if gap <= 1e-5 {
                    let normal = if distance > 1e-7 {
                        scale(diff, 1. / distance)
                    } else {
                        unit(cross(sub(tri[1], tri[0]), sub(tri[2], tri[0])))
                    };
                    if dot(motion, normal) < -1e-7 && result.as_ref().is_none_or(|r| t < r.fraction)
                    {
                        result = Some(ZoneSweepHit {
                            moving,
                            surface,
                            fraction: t,
                            point: b,
                            normal,
                        });
                    }
                    break;
                }
                t += gap / speed;
                if t > 1. || result.as_ref().is_some_and(|r| t >= r.fraction) {
                    break;
                }
            }
        });
        result
    }
}
pub fn on(world: &World, owner: ComponentId) -> Option<std::sync::Arc<TriangleMesh>> {
    world.children_of(owner).iter().find_map(|&id| {
        world
            .get_component_by_id_as::<CutGeometryComponent>(id)
            .map(|g| g.mesh.clone())
    })
}
fn capsule(shape: CollisionShape) -> (f32, f32) {
    match shape {
        CollisionShape::CapsuleY {
            radius,
            half_segment,
        } => (radius, half_segment),
        CollisionShape::Sphere { radius } => (radius, 0.),
        CollisionShape::Cube { .. } => (0., 0.),
    }
}
fn ray_triangle(o: [f32; 3], d: [f32; 3], p: [[f32; 3]; 3]) -> Option<f32> {
    let e = sub(p[1], p[0]);
    let f = sub(p[2], p[0]);
    let h = cross(d, f);
    let det = dot(e, h);
    if det.abs() < 1e-9 {
        return None;
    }
    let s = sub(o, p[0]);
    let u = dot(s, h) / det;
    if !(0. ..=1.).contains(&u) {
        return None;
    }
    let q = cross(s, e);
    let v = dot(d, q) / det;
    if v < 0. || u + v > 1. {
        return None;
    }
    let t = dot(f, q) / det;
    (t >= 0.).then_some(t)
}
fn closest(p: [f32; 3], t: [[f32; 3]; 3]) -> [f32; 3] {
    let [a, b, c] = t;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0. && d2 <= 0. {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0. && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return add(a, scale(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0. && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return add(a, scale(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && (d4 - d3) >= 0. && (d5 - d6) >= 0. {
        return add(b, scale(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let denominator = 1. / (va + vb + vc);
    add(
        a,
        add(scale(ab, vb * denominator), scale(ac, vc * denominator)),
    )
}
fn segments(p: [f32; 3], q: [f32; 3], a: [f32; 3], b: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let d = sub(q, p);
    let e = sub(b, a);
    let r = sub(p, a);
    let aa = dot(d, d);
    let ee = dot(e, e);
    let f = dot(e, r);
    let mut s = if aa < 1e-12 {
        0.
    } else {
        let c = dot(d, r);
        let be = dot(d, e);
        let den = aa * ee - be * be;
        if den.abs() > 1e-12 {
            ((be * f - c * ee) / den).clamp(0., 1.)
        } else {
            0.
        }
    };
    let t = if ee < 1e-12 {
        0.
    } else {
        ((dot(d, e) * s + f) / ee).clamp(0., 1.)
    };
    if aa > 1e-12 {
        s = ((dot(d, e) * t - dot(d, r)) / aa).clamp(0., 1.);
    }
    (add(p, scale(d, s)), add(a, scale(e, t)))
}
fn segment_triangle(a: [f32; 3], b: [f32; 3], tri: [[f32; 3]; 3]) -> ([f32; 3], [f32; 3]) {
    if let Some(t) = ray_triangle(a, sub(b, a), tri) {
        if t <= 1. {
            let p = add(a, scale(sub(b, a), t));
            return (p, p);
        }
    }
    let mut best = (a, closest(a, tri));
    let candidate = (b, closest(b, tri));
    if dot(sub(candidate.0, candidate.1), sub(candidate.0, candidate.1))
        < dot(sub(best.0, best.1), sub(best.0, best.1))
    {
        best = candidate;
    }
    for i in 0..3 {
        let candidate = segments(a, b, tri[i], tri[(i + 1) % 3]);
        if dot(sub(candidate.0, candidate.1), sub(candidate.0, candidate.1))
            < dot(sub(best.0, best.1), sub(best.0, best.1))
        {
            best = candidate;
        }
    }
    best
}
fn box_triangle(center: [f32; 3], ext: [f32; 3], tri: [[f32; 3]; 3]) -> Option<[f32; 3]> {
    let tri = tri.map(|p| sub(p, center));
    let edges = [
        sub(tri[1], tri[0]),
        sub(tri[2], tri[1]),
        sub(tri[0], tri[2]),
    ];
    let basis = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
    let mut axes = [[0.; 3]; 13];
    axes[..3].copy_from_slice(&basis);
    axes[3] = cross(edges[0], edges[1]);
    for i in 0..3 {
        for j in 0..3 {
            axes[4 + i * 3 + j] = cross(edges[i], basis[j]);
        }
    }
    let mut depth = f32::INFINITY;
    let mut result = [0.; 3];
    for axis in axes {
        if dot(axis, axis) < 1e-12 {
            continue;
        }
        let n = unit(axis);
        let projected = tri.map(|p| dot(p, n));
        let low = projected.into_iter().fold(f32::INFINITY, f32::min);
        let high = projected.into_iter().fold(f32::NEG_INFINITY, f32::max);
        let radius = (0..3).map(|i| ext[i] * n[i].abs()).sum::<f32>();
        if low >= radius || high <= -radius {
            return None;
        }
        let delta = if high + radius < radius - low {
            high + radius
        } else {
            low - radius
        };
        if delta.abs() < depth {
            depth = delta.abs();
            result = scale(n, delta);
        }
    }
    depth.is_finite().then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::csg_mesh::Solid;
    fn floor() -> TriangleMesh {
        let solid = Solid::box_between([-2., -0.2, -2.], [2., 0., 2.]);
        let cutter = Solid::box_between([-0.6, -1., -0.6], [0.6, 1., 0.6]);
        TriangleMesh::new(&solid.cut(&cutter).unwrap().0.mesh())
    }
    #[test]
    fn hole_has_no_ray_or_capsule_collision_but_rim_does() {
        let floor = floor();
        assert!(floor.ray([0., 1., 0.], [0., -1., 0.]).is_none());
        assert!((floor.ray([1., 1., 0.], [0., -1., 0.]).unwrap() - 1.).abs() < 1e-5);
        let shape = CollisionShape::capsule_y(0.1, 0.4);
        assert!(floor.translation([0., 0., 0.], shape).is_none());
        assert!(floor.translation([0.58, 0., 0.], shape).is_some());
        assert!(floor.translation([1., 0.45, 0.], shape).unwrap()[1] > 0.);
        assert!(
            floor
                .sweep(
                    ComponentId::default(),
                    ComponentId::default(),
                    [0., -2., 0.],
                    shape,
                    [0., 2., 0.]
                )
                .is_none()
        );
        let hit = floor
            .sweep(
                ComponentId::default(),
                ComponentId::default(),
                [1., -2., 0.],
                shape,
                [1., 2., 0.],
            )
            .unwrap();
        assert!((hit.fraction - 0.375).abs() < 1e-3);
        assert!(hit.normal[1] > 0.99);
    }
    #[test]
    fn fast_capsule_hits_wall_and_box_contacts_work() {
        let wall = TriangleMesh::new(&Solid::box_between([-2., -2., -0.1], [2., 2., 0.1]).mesh());
        let shape = CollisionShape::capsule_y(0.1, 0.3);
        let hit = wall
            .sweep(
                ComponentId::default(),
                ComponentId::default(),
                [0., 0., -3.],
                shape,
                [0., 0., 3.],
            )
            .unwrap();
        assert!(hit.normal[2] > 0.99);
        assert!(
            wall.translation([0., 0., 0.15], CollisionShape::cube_half_extents([0.1; 3]))
                .unwrap()[2]
                > 0.
        );
    }
}
