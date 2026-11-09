//! Strict geometry import at the solid-kernel boundary. Never repairs open
//! panels into boxes or welds nearby points across real gaps.
use super::CpuMesh;
use manifold_rust::{manifold::Manifold, types::MeshGL64};
use std::collections::HashMap;

const MAX_SOURCE_TRIANGLES: usize = 16_384;

pub(super) fn from_cpu_mesh(mesh: &CpuMesh) -> Result<Manifold, String> {
    if mesh.joints0.is_some() || mesh.weights0.is_some() || !mesh.morph_targets.is_empty() {
        return Err("CSG import currently supports rigid meshes only; skinning and morph targets need an explicit deformation policy".into());
    }
    if mesh.indices_u32.is_empty() || mesh.indices_u32.len() % 3 != 0 {
        return Err("CSG import requires a nonempty triangle index list".into());
    }
    if mesh.indices_u32.len() / 3 > MAX_SOURCE_TRIANGLES
        || mesh.vertices.len() > MAX_SOURCE_TRIANGLES * 3
    {
        return Err(
            "CSG import exceeds the 16384-triangle source budget; partition the mesh first".into(),
        );
    }
    if mesh
        .vertices
        .iter()
        .any(|v| !v.pos.into_iter().all(f32::is_finite))
    {
        return Err("CSG import contains non-finite vertex positions".into());
    }

    // Render meshes duplicate positions at UV/normal seams. Canonicalize exact
    // positions for solid topology, without mutating their rendering attributes.
    // Treat signed zero equally; no approximate weld can close an authored gap.
    let mut positions = HashMap::<[u32; 3], u64>::new();
    let mut input = MeshGL64 {
        num_prop: 3,
        ..Default::default()
    };
    input.tri_verts.reserve(mesh.indices_u32.len());
    for &index in &mesh.indices_u32 {
        let vertex = mesh
            .vertices
            .get(index as usize)
            .ok_or_else(|| format!("CSG import index {index} is outside the vertex array"))?;
        let key = vertex.pos.map(|v| if v == 0.0 { 0 } else { v.to_bits() });
        let next = positions.len() as u64;
        let canonical = *positions.entry(key).or_insert_with(|| {
            input.vert_properties.extend(vertex.pos.map(f64::from));
            next
        });
        input.tri_verts.push(canonical);
    }

    let mut edges = HashMap::<(u64, u64), (usize, i32)>::new();
    let origin = input.get_vert_pos(0);
    let mut signed_volume6 = 0.0;
    for (triangle, indices) in input.tri_verts.chunks_exact(3).enumerate() {
        let p: [[f64; 3]; 3] = std::array::from_fn(|i| input.get_vert_pos(indices[i] as usize));
        let a: [f64; 3] = std::array::from_fn(|i| p[1][i] - p[0][i]);
        let b: [f64; 3] = std::array::from_fn(|i| p[2][i] - p[0][i]);
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        if cross.into_iter().map(|v| v * v).sum::<f64>() == 0.0 {
            return Err(format!("CSG import triangle {triangle} is degenerate"));
        }
        signed_volume6 += (0..3)
            .map(|i| (p[0][i] - origin[i]) * cross[i])
            .sum::<f64>();
        for (a, b) in [
            (indices[0], indices[1]),
            (indices[1], indices[2]),
            (indices[2], indices[0]),
        ] {
            let (key, direction) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += direction;
        }
    }
    // Iteration order does not affect the diagnostic class.
    if edges.values().any(|&(count, _)| count == 1) {
        return Err("CSG import has open boundary edges; author a closed volume or use an explicit thin-panel strategy".into());
    }
    if edges.values().any(|&(count, _)| count != 2) {
        return Err(
            "CSG import has non-manifold edges; split or repair the source geometry".into(),
        );
    }
    if edges.values().any(|&(_, direction)| direction != 0) {
        return Err("CSG import has inconsistent triangle winding".into());
    }
    if signed_volume6 <= 0.0 {
        return Err("CSG import requires a positive closed volume with outward winding".into());
    }
    // Keep property vertices distinct at UV seams, while the explicit merge
    // map supplies the same exact-position topology validated above. Manifold
    // interpolates these channels when a boolean splits an authored triangle.
    let mut attributed = MeshGL64 {
        num_prop: 6,
        ..Default::default()
    };
    let mut properties = HashMap::<u32, u64>::new();
    let mut geometry = HashMap::<[u32; 3], u64>::new();
    for &index in &mesh.indices_u32 {
        let vertex = &mesh.vertices[index as usize];
        if !vertex.uv.into_iter().all(f32::is_finite) {
            return Err("CSG import contains non-finite texture coordinates".into());
        }
        let next = attributed.vert_properties.len() as u64 / 6;
        let property = *properties.entry(index).or_insert_with(|| {
            attributed.vert_properties.extend(vertex.pos.map(f64::from));
            attributed.vert_properties.extend(vertex.uv.map(f64::from));
            // Generated cutter surfaces have zero in this channel; use the
            // source's stable cut-face sample there instead of planar UVs.
            attributed.vert_properties.push(1.0);
            let key = vertex.pos.map(|v| if v == 0.0 { 0 } else { v.to_bits() });
            if let Some(&canonical) = geometry.get(&key) {
                attributed.merge_from_vert.push(next);
                attributed.merge_to_vert.push(canonical);
            } else {
                geometry.insert(key, next);
            }
            next
        });
        attributed.tri_verts.push(property);
    }
    Ok(Manifold::from_mesh_gl64(&attributed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::{
        csg_mesh::{Operation, Solid},
        mesh::CpuVertex,
    };

    fn irregular_mesh() -> CpuMesh {
        Solid::box_between([0., 0., 0.], [2., 1., 1.])
            .boolean(
                &Solid::box_between([0., 0., 0.], [1., 2., 1.]),
                Operation::Union,
            )
            .unwrap()
            .mesh()
    }

    #[test]
    fn closed_geometry_remains_cuttable_after_rigid_rotation_and_nonuniform_scale() {
        let solid = Solid::from_mesh(&irregular_mesh()).unwrap();
        let transform = [
            [0.0, 0.0, -2.0, 0.0],
            [0.0, 0.5, 0.0, 0.0],
            [1.5, 0.0, 0.0, 0.0],
            [5.0, 2.0, -3.0, 1.0],
        ];
        let rotated = Solid::from_mesh(&solid.transformed(transform).mesh()).unwrap();
        let cutter = Solid::box_between([0.2, 0.2, -0.1], [0.6, 0.6, 1.1]).transformed(transform);
        let (rest, pieces) = rotated.cut(&cutter).unwrap();
        assert!((rest.volume() - 2.84 * 1.5).abs() < 1e-5);
        assert!((pieces.iter().map(Solid::volume).sum::<f64>() - 0.16 * 1.5).abs() < 1e-5);
    }

    #[test]
    fn disconnected_closed_shells_remain_separate_volumes() {
        let mesh = Solid::box_between([0.; 3], [1.; 3])
            .boolean(&Solid::box_between([2.; 3], [3.; 3]), Operation::Union)
            .unwrap()
            .mesh();
        let solid = Solid::from_mesh(&mesh).unwrap();
        let (remaining, pieces) = solid
            .cut(&Solid::box_between([0.; 3], [0.5, 1., 1.]))
            .unwrap();
        assert!((remaining.volume() - 1.5).abs() < 1e-7);
        assert!((pieces.iter().map(Solid::volume).sum::<f64>() - 0.5).abs() < 1e-7);
        assert!(Solid::from_mesh(&remaining.mesh()).is_ok());
    }

    #[test]
    fn imports_seamed_irregular_geometry_and_cuts_actual_shape() {
        let mesh = irregular_mesh();
        let original_positions: Vec<_> = mesh.vertices.iter().map(|v| v.pos).collect();
        let solid = Solid::from_mesh(&mesh).unwrap();
        assert!((solid.volume() - 3.0).abs() < 1e-7);
        // This cutter is inside the bounding box, but outside the L-shaped mesh.
        let gap = Solid::box_between([1.2, 1.2, 0.2], [1.8, 1.8, 0.8]);
        let (unchanged, debris) = solid.cut(&gap).unwrap();
        assert!(debris.is_empty());
        assert!((unchanged.volume() - 3.0).abs() < 1e-7);
        let cutter = Solid::box_between([0.2, 0.2, -0.1], [0.6, 0.6, 1.1]);
        let (rest, debris) = solid.cut(&cutter).unwrap();
        assert!((rest.volume() - 2.84).abs() < 1e-6);
        assert!((rest.volume() + debris.iter().map(Solid::volume).sum::<f64>() - 3.0).abs() < 1e-6);
        assert!(Solid::from_mesh(&rest.mesh()).is_ok());
        assert_eq!(
            original_positions,
            mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()
        );
        assert!((Solid::from_mesh(&mesh).unwrap().volume() - 3.0).abs() < 1e-7);
    }

    #[test]
    fn rejects_invalid_open_non_manifold_and_inverted_geometry() {
        let closed = irregular_mesh();
        let error = |mesh: &CpuMesh| Solid::from_mesh(mesh).unwrap_err();
        let mut mesh = closed.clone();
        mesh.indices_u32.truncate(mesh.indices_u32.len() - 3);
        assert!(error(&mesh).contains("open boundary"));
        let mut mesh = closed.clone();
        mesh.indices_u32.swap(0, 1);
        assert!(error(&mesh).contains("winding"));
        let mut mesh = closed.clone();
        for face in mesh.indices_u32.chunks_exact_mut(3) {
            face.swap(0, 1);
        }
        assert!(error(&mesh).contains("positive closed volume"));
        let mut mesh = closed.clone();
        mesh.indices_u32.extend_from_within(0..3);
        assert!(error(&mesh).contains("non-manifold"));
        let mut mesh = closed.clone();
        mesh.indices_u32[0] = u32::MAX;
        assert!(error(&mesh).contains("outside"));
        let mut mesh = closed.clone();
        mesh.vertices[0].pos[0] = f32::NAN;
        assert!(error(&mesh).contains("non-finite"));
        let mut mesh = closed.clone();
        mesh.indices_u32[1] = mesh.indices_u32[0];
        assert!(error(&mesh).contains("degenerate"));
        let mut mesh = closed.clone();
        mesh.indices_u32.push(0);
        assert!(error(&mesh).contains("triangle index list"));
        let mut mesh = closed.clone();
        mesh.joints0 = Some(vec![[0; 4]; mesh.vertices.len()]);
        assert!(error(&mesh).contains("rigid meshes only"));
        let mut mesh = closed.clone();
        mesh.indices_u32 = vec![0; (MAX_SOURCE_TRIANGLES + 1) * 3];
        assert!(error(&mesh).contains("budget"));
        assert!(error(&CpuMesh::new(Vec::new(), Vec::new())).contains("nonempty"));
        let mut mesh = closed;
        mesh.vertices.push(CpuVertex {
            pos: [-0.0, 0.0, 0.0],
            ..Default::default()
        });
        assert!(Solid::from_mesh(&mesh).is_ok());
    }
}
