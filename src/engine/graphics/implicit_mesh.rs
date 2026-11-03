use std::collections::HashMap;
use std::fmt;

use mcubes::{MarchingCubes, MeshSide};

use crate::engine::graphics::mesh::{CpuMesh, CpuVertex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImplicitGridSpec {
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub cells: [usize; 3],
    pub iso_level: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplicitMeshError {
    InvalidGrid(String),
    InvalidSamples(String),
    Backend(String),
    InvalidOutput(String),
}

impl fmt::Display for ImplicitMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrid(message) => write!(f, "invalid implicit grid: {message}"),
            Self::InvalidSamples(message) => write!(f, "invalid implicit samples: {message}"),
            Self::Backend(message) => write!(f, "marching-cubes backend failed: {message}"),
            Self::InvalidOutput(message) => write!(f, "invalid implicit mesh output: {message}"),
        }
    }
}

impl std::error::Error for ImplicitMeshError {}

fn checked_node_count(cells: [usize; 3]) -> Result<usize, ImplicitMeshError> {
    cells
        .into_iter()
        .map(|cells| cells.checked_add(1))
        .try_fold(1usize, |count, nodes| {
            count
                .checked_mul(nodes.ok_or_else(|| {
                    ImplicitMeshError::InvalidGrid("sample dimension overflow".into())
                })?)
                .ok_or_else(|| ImplicitMeshError::InvalidGrid("sample count overflow".into()))
        })
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn validate_closed(indices: &[u32]) -> Result<(), ImplicitMeshError> {
    let mut edges: HashMap<(u32, u32), (u32, i32)> = HashMap::new();
    for triangle in indices.chunks_exact(3) {
        for (from, to) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let (key, direction) = if from < to {
                ((from, to), 1)
            } else {
                ((to, from), -1)
            };
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += direction;
        }
    }
    if let Some(((a, b), (uses, direction))) = edges
        .into_iter()
        .find(|(_, (uses, direction))| *uses != 2 || *direction != 0)
    {
        return Err(ImplicitMeshError::InvalidOutput(format!(
            "edge ({a}, {b}) has {uses} uses and directed balance {direction}; expected two opposing uses"
        )));
    }
    Ok(())
}

/// Convert a sampled scalar grid to Mittens' indexed triangle mesh contract.
///
/// Samples use the backend-observed order `x + y * nx + z * nx * ny`.
pub fn extract_implicit_mesh(
    spec: &ImplicitGridSpec,
    samples: Vec<f32>,
) -> Result<CpuMesh, ImplicitMeshError> {
    for axis in 0..3 {
        if spec.cells[axis] == 0 {
            return Err(ImplicitMeshError::InvalidGrid(format!(
                "axis {axis} has zero cells"
            )));
        }
        if !spec.bounds_min[axis].is_finite()
            || !spec.bounds_max[axis].is_finite()
            || spec.bounds_min[axis] >= spec.bounds_max[axis]
        {
            return Err(ImplicitMeshError::InvalidGrid(format!(
                "axis {axis} bounds must be finite and increasing"
            )));
        }
    }
    if !spec.iso_level.is_finite() {
        return Err(ImplicitMeshError::InvalidGrid(
            "iso level must be finite".into(),
        ));
    }

    let expected = checked_node_count(spec.cells)?;
    if samples.len() != expected {
        return Err(ImplicitMeshError::InvalidSamples(format!(
            "got {} samples, expected {expected}",
            samples.len()
        )));
    }
    if let Some(index) = samples.iter().position(|sample| !sample.is_finite()) {
        return Err(ImplicitMeshError::InvalidSamples(format!(
            "sample {index} is not finite"
        )));
    }

    let extent = [
        spec.bounds_max[0] - spec.bounds_min[0],
        spec.bounds_max[1] - spec.bounds_min[1],
        spec.bounds_max[2] - spec.bounds_min[2],
    ];
    let nodes = [spec.cells[0] + 1, spec.cells[1] + 1, spec.cells[2] + 1];
    let backend = MarchingCubes::new(
        (nodes[0], nodes[1], nodes[2]),
        (extent[0], extent[1], extent[2]),
        (
            spec.cells[0] as f32,
            spec.cells[1] as f32,
            spec.cells[2] as f32,
        ),
        Default::default(),
        samples,
        spec.iso_level,
    )
    .map_err(|error| ImplicitMeshError::Backend(error.to_string()))?;
    let output = backend.generate(MeshSide::OutsideOnly);
    if output.indices.is_empty() {
        return Ok(CpuMesh::new(Vec::new(), Vec::new()));
    }
    if output.indices.len() % 3 != 0 {
        return Err(ImplicitMeshError::InvalidOutput(format!(
            "index count {} is not divisible by three",
            output.indices.len()
        )));
    }

    // mcubes emits one vertex per triangle corner. Weld the same cell-edge
    // intersections deterministically before validating manifold topology.
    let min_cell = (0..3)
        .map(|axis| extent[axis] / spec.cells[axis] as f32)
        .fold(f32::INFINITY, f32::min);
    // The backend independently interpolates each triangle corner. Complex
    // multi-cell fields can differ by a few ten-thousandths of one cell for
    // what is mathematically the same edge crossing, so quantize far below
    // renderer precision while keeping adjacent grid features distinct.
    let weld_step = (min_cell * 1.0e-4).max(f32::EPSILON * 32.0);
    let mut weld_map: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices = Vec::with_capacity(output.indices.len());

    for backend_index in output.indices {
        let vertex = output.vertices.get(backend_index).ok_or_else(|| {
            ImplicitMeshError::InvalidOutput(format!(
                "backend index {backend_index} exceeds {} vertices",
                output.vertices.len()
            ))
        })?;
        let position = [
            vertex.posit.x + spec.bounds_min[0],
            vertex.posit.y + spec.bounds_min[1],
            vertex.posit.z + spec.bounds_min[2],
        ];
        if !position.into_iter().all(f32::is_finite) {
            return Err(ImplicitMeshError::InvalidOutput(
                "backend emitted a non-finite position".into(),
            ));
        }
        let bucket = [
            ((position[0] - spec.bounds_min[0]) / weld_step).floor() as i64,
            ((position[1] - spec.bounds_min[1]) / weld_step).floor() as i64,
            ((position[2] - spec.bounds_min[2]) / weld_step).floor() as i64,
        ];
        // Check neighboring buckets as well: rounding a coordinate into one
        // hash key is not a valid tolerance test when two values straddle a
        // bucket boundary.
        let mut welded = None;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let neighbor = [bucket[0] + dx, bucket[1] + dy, bucket[2] + dz];
                    if let Some(candidates) = weld_map.get(&neighbor) {
                        for &candidate in candidates {
                            let existing = positions[candidate as usize];
                            if (existing[0] - position[0]).abs() <= weld_step
                                && (existing[1] - position[1]).abs() <= weld_step
                                && (existing[2] - position[2]).abs() <= weld_step
                            {
                                welded =
                                    Some(welded.map_or(candidate, |old: u32| old.min(candidate)));
                            }
                        }
                    }
                }
            }
        }
        let index = if let Some(index) = welded {
            index
        } else {
            let index = u32::try_from(positions.len()).map_err(|_| {
                ImplicitMeshError::InvalidOutput("more than u32::MAX vertices".into())
            })?;
            positions.push(position);
            weld_map.entry(bucket).or_default().push(index);
            index
        };
        indices.push(index);
    }

    let mut filtered_indices = Vec::with_capacity(indices.len());
    for triangle in indices.chunks_exact(3) {
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0] {
            continue;
        }
        let a = positions[triangle[0] as usize];
        let b = positions[triangle[1] as usize];
        let c = positions[triangle[2] as usize];
        let face = cross(sub(b, a), sub(c, a));
        let area_squared = dot(face, face);
        if !area_squared.is_finite() {
            return Err(ImplicitMeshError::InvalidOutput(
                "backend emitted a triangle with non-finite area".into(),
            ));
        }
        if area_squared > 0.0 {
            filtered_indices.extend_from_slice(triangle);
        }
    }
    indices = filtered_indices;
    if indices.is_empty() {
        return Err(ImplicitMeshError::InvalidOutput(
            "surface crossing produced only degenerate triangles".into(),
        ));
    }

    // Remove vertices referenced only by discarded degenerate faces while
    // retaining deterministic first-use order.
    let mut remap = vec![u32::MAX; positions.len()];
    let mut compact_positions = Vec::new();
    for index in &mut indices {
        let old = *index as usize;
        if remap[old] == u32::MAX {
            remap[old] = u32::try_from(compact_positions.len()).map_err(|_| {
                ImplicitMeshError::InvalidOutput("more than u32::MAX vertices".into())
            })?;
            compact_positions.push(positions[old]);
        }
        *index = remap[old];
    }
    positions = compact_positions;

    let signed_volume_six: f64 = indices
        .chunks_exact(3)
        .map(|triangle| {
            let a = positions[triangle[0] as usize].map(f64::from);
            let b = positions[triangle[1] as usize].map(f64::from);
            let c = positions[triangle[2] as usize].map(f64::from);
            a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0])
        })
        .sum();
    if signed_volume_six == 0.0 || !signed_volume_six.is_finite() {
        return Err(ImplicitMeshError::InvalidOutput(
            "mesh has zero or non-finite signed volume".into(),
        ));
    }
    if signed_volume_six < 0.0 {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    validate_closed(&indices)?;

    let mut normal_sums = vec![[0.0f32; 3]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let a = positions[triangle[0] as usize];
        let b = positions[triangle[1] as usize];
        let c = positions[triangle[2] as usize];
        let normal = cross(sub(b, a), sub(c, a));
        for &index in triangle {
            for axis in 0..3 {
                normal_sums[index as usize][axis] += normal[axis];
            }
        }
    }

    let vertices = positions
        .into_iter()
        .zip(normal_sums)
        .map(|(pos, normal)| {
            let length = dot(normal, normal).sqrt();
            if !length.is_finite() || length <= f32::EPSILON {
                return Err(ImplicitMeshError::InvalidOutput(
                    "could not construct a finite vertex normal".into(),
                ));
            }
            Ok(CpuVertex {
                pos,
                normal: [normal[0] / length, normal[1] / length, normal[2] / length],
                uv: [pos[0], pos[2]],
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CpuMesh::new(vertices, indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere_fixture(cells: usize) -> (ImplicitGridSpec, Vec<f32>) {
        let spec = ImplicitGridSpec {
            bounds_min: [-1.5; 3],
            bounds_max: [1.5; 3],
            cells: [cells; 3],
            iso_level: 0.0,
        };
        let nodes = cells + 1;
        let mut samples = vec![0.0; nodes * nodes * nodes];
        for z in 0..nodes {
            for y in 0..nodes {
                for x in 0..nodes {
                    let p = [
                        -1.5 + 3.0 * x as f32 / cells as f32,
                        -1.5 + 3.0 * y as f32 / cells as f32,
                        -1.5 + 3.0 * z as f32 / cells as f32,
                    ];
                    samples[x + y * nodes + z * nodes * nodes] =
                        (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 1.0;
                }
            }
        }
        (spec, samples)
    }

    #[test]
    fn extracts_closed_outward_deterministic_sphere() {
        let (spec, samples) = sphere_fixture(20);
        let first = extract_implicit_mesh(&spec, samples.clone()).unwrap();
        let second = extract_implicit_mesh(&spec, samples).unwrap();
        assert!(!first.vertices.is_empty());
        assert_eq!(first.indices_u32, second.indices_u32);
        assert_eq!(
            first.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
            second.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()
        );
        validate_closed(&first.indices_u32).unwrap();
        for vertex in &first.vertices {
            assert!(dot(vertex.pos, vertex.normal) > 0.0);
        }
    }

    #[test]
    fn all_outside_is_a_valid_empty_mesh() {
        let spec = ImplicitGridSpec {
            bounds_min: [-1.0; 3],
            bounds_max: [1.0; 3],
            cells: [4; 3],
            iso_level: 0.0,
        };
        let mesh = extract_implicit_mesh(&spec, vec![1.0; 125]).unwrap();
        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices_u32.is_empty());
    }

    #[test]
    fn rejects_shape_mismatch_and_non_finite_samples() {
        let spec = ImplicitGridSpec {
            bounds_min: [-1.0; 3],
            bounds_max: [1.0; 3],
            cells: [2; 3],
            iso_level: 0.0,
        };
        assert!(extract_implicit_mesh(&spec, vec![1.0; 26]).is_err());
        let mut samples = vec![1.0; 27];
        samples[10] = f32::NAN;
        assert!(extract_implicit_mesh(&spec, samples).is_err());
    }
}
