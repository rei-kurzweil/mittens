//! Closed full tori and capped torus arcs, with flat rectangular four-sided profiles.
use super::{CpuMesh, CpuVertex, MeshFactory};
impl MeshFactory {
    pub fn torus(
        radius: f32,
        tube_radius: f32,
        segments: u32,
        sides: u32,
    ) -> Result<CpuMesh, String> {
        if segments < 3 {
            return Err("torus requires at least three segments".into());
        }
        Self::partial_torus(
            radius,
            tube_radius,
            0.0,
            std::f32::consts::TAU,
            segments,
            sides,
        )
    }
    pub fn partial_torus(
        radius: f32,
        tube_radius: f32,
        start: f32,
        sweep: f32,
        segments: u32,
        sides: u32,
    ) -> Result<CpuMesh, String> {
        if !start.is_finite()
            || !sweep.is_finite()
            || sweep <= 0.0
            || sweep > std::f32::consts::TAU
            || !radius.is_finite()
            || !tube_radius.is_finite()
            || tube_radius <= 0.0
            || radius <= tube_radius
            || !(1..=16384).contains(&segments)
            || !(3..=64).contains(&sides)
            || u64::from(segments + 1) * u64::from(sides) * 2 + 2 * u64::from(sides + 1) > 1_000_000
        {
            return Err("partial_torus requires finite radius > tube_radius > 0, 0 < sweep <= TAU, 1..=16384 segments and 3..=64 sides (at most one million vertices)".into());
        }
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let phase = if sides == 4 {
            std::f32::consts::FRAC_PI_4
        } else {
            0.0
        };
        for side in 0..sides {
            let a = phase + std::f32::consts::TAU * side as f32 / sides as f32;
            let b = phase + std::f32::consts::TAU * (side + 1) as f32 / sides as f32;
            let base = vertices.len() as u32;
            for i in 0..=segments {
                let u = i as f32 / segments as f32;
                let theta = start + sweep * u;
                let (sin, cos) = theta.sin_cos();
                for (edge, angle) in [a, b].into_iter().enumerate() {
                    let normal_angle = if sides == 4 { (a + b) * 0.5 } else { angle };
                    let r = radius + tube_radius * angle.cos();
                    vertices.push(CpuVertex {
                        pos: [r * cos, tube_radius * angle.sin(), r * sin],
                        normal: [
                            normal_angle.cos() * cos,
                            normal_angle.sin(),
                            normal_angle.cos() * sin,
                        ],
                        uv: [u, (side as f32 + edge as f32) / sides as f32],
                    });
                }
            }
            for i in 0..segments {
                let a = base + i * 2;
                // Profile direction crossed with circumference points outward.
                indices.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
            }
        }
        if sweep < std::f32::consts::TAU - 1e-5 {
            for (theta, end) in [(start, false), (start + sweep, true)] {
                let (sin, cos) = theta.sin_cos();
                let tangent = [-sin, 0.0, cos];
                let sign = if end { 1.0 } else { -1.0 };
                let normal = tangent.map(|v| v * sign);
                let base = vertices.len() as u32;
                vertices.push(CpuVertex {
                    pos: [radius * cos, 0., radius * sin],
                    normal,
                    uv: [0.5, 0.5],
                });
                for side in 0..sides {
                    let angle = phase + std::f32::consts::TAU * side as f32 / sides as f32;
                    let r = radius + tube_radius * angle.cos();
                    vertices.push(CpuVertex {
                        pos: [r * cos, tube_radius * angle.sin(), r * sin],
                        normal,
                        uv: [0.5 + 0.5 * angle.cos(), 0.5 + 0.5 * angle.sin()],
                    });
                }
                for side in 0..sides {
                    let a = base + 1 + side;
                    let b = base + 1 + (side + 1) % sides;
                    let triangle = if end { [base, a, b] } else { [base, b, a] };
                    indices.extend_from_slice(&triangle);
                }
            }
        }
        Ok(CpuMesh::new(vertices, indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::math::{vec3_cross, vec3_sub};
    #[test]
    fn partial_tori_are_capped_watertight_and_outward_wound() {
        for sides in [4, 12] {
            let mesh = MeshFactory::partial_torus(0.4, 0.04, -0.3, 2.2, 24, sides).unwrap();
            assert_eq!(
                mesh.indices_u32.len(),
                (24 * sides * 6 + 2 * sides * 3) as usize
            );
            let mut edges = std::collections::BTreeMap::new();
            let point = |p: [f32; 3]| p.map(|v| (v * 100_000.).round() as i32);
            for triangle in mesh.indices_u32.chunks_exact(3) {
                let v = [triangle[0], triangle[1], triangle[2]].map(|i| mesh.vertices[i as usize]);
                let normal = vec3_cross(vec3_sub(v[1].pos, v[0].pos), vec3_sub(v[2].pos, v[0].pos));
                assert!(
                    normal
                        .into_iter()
                        .zip(v[0].normal)
                        .map(|(a, b)| a * b)
                        .sum::<f32>()
                        > 1e-8
                );
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let (a, b) = (point(v[a].pos), point(v[b].pos));
                    *edges
                        .entry(if a < b { (a, b) } else { (b, a) })
                        .or_insert(0) += 1;
                }
            }
            assert!(
                edges.values().all(|&count| count == 2),
                "every welded edge must have two faces"
            );
        }
        assert!(MeshFactory::partial_torus(0.4, 0.04, 0., 0., 8, 4).is_err());
        assert!(MeshFactory::partial_torus(0.4, 0.04, 0., 7., 8, 4).is_err());
    }
}
