//! Exercises the rigid CSG boundary with the same immutable GLB mesh data used
//! by imported assets. This is not a test of live Destroyable publication yet.
use mittens_engine::engine::{
    ecs::{
        ComponentId,
        system::csg_system::{CsgSystem, MeshCutJob},
    },
    graphics::{
        csg_mesh::Solid,
        mesh::{CpuMesh, CpuVertex},
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
fn pickup_glb_closed_parts_can_be_submitted_without_box_substitution() {
    let gltf =
        gltf::Gltf::from_slice(include_bytes!("../assets/models/mitten_pickup.glb")).unwrap();
    let blob = gltf.blob.as_deref().unwrap();
    let system = CsgSystem::default();
    let mut accepted = 0;
    let mut rejected = 0;
    for mesh in gltf.meshes() {
        for (primitive_index, primitive) in mesh.primitives().enumerate() {
            assert_eq!(primitive.mode(), gltf::mesh::Mode::Triangles);
            let reader = primitive.reader(|buffer| match buffer.source() {
                gltf::buffer::Source::Bin => Some(blob),
                gltf::buffer::Source::Uri(_) => None,
            });
            assert!(reader.read_joints(0).is_none() && reader.read_weights(0).is_none());
            assert!(primitive.morph_targets().next().is_none());
            let vertices: Vec<_> = reader
                .read_positions()
                .unwrap()
                .map(|pos| CpuVertex {
                    pos,
                    ..Default::default()
                })
                .collect();
            let indices = reader.read_indices().unwrap().into_u32().collect();
            let source = Arc::new(CpuMesh::new(vertices, indices));
            let bounds = mittens_engine::engine::graphics::bounds::Aabb::from_points(
                &source.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
            )
            .unwrap();
            // Center the cutter on an actual triangle, not empty cab space.
            // Open primitives must still be rejected before cutting.
            let center: [f32; 3] = std::array::from_fn(|axis| {
                source.indices_u32[..3]
                    .iter()
                    .map(|&i| source.vertices[i as usize].pos[axis])
                    .sum::<f32>()
                    / 3.0
            });
            let radius: [f32; 3] =
                std::array::from_fn(|i| ((bounds.max[i] - bounds.min[i]) * 0.10).max(0.001));
            let cutter = Solid::box_between(
                std::array::from_fn(|i| center[i] - radius[i]),
                std::array::from_fn(|i| center[i] + radius[i]),
            );
            system
                .submit_mesh(MeshCutJob {
                    owner: ComponentId::default(),
                    revision: mesh.index() as u64,
                    mesh: source.clone(),
                    cutter,
                })
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            let result = loop {
                if let Some(result) = system.poll() {
                    break result;
                }
                assert!(Instant::now() < deadline, "pickup primitive cut timed out");
                std::thread::sleep(Duration::from_millis(2));
            };
            let name = format!("{}:prim{primitive_index}", mesh.name().unwrap_or("unnamed"));
            match result.result {
                Ok(batch) => {
                    accepted += 1;
                    let original = Solid::from_mesh(&source).unwrap();
                    assert_eq!(
                        batch.cells.len(),
                        1,
                        "{name}: cutter must remove actual material"
                    );
                    let remaining = &batch.cells[0].solid;
                    assert!(remaining.volume() < original.volume());
                    assert!(remaining.volume() > 0.0);
                    let removed: f64 = batch
                        .debris
                        .iter()
                        .map(|mesh| Solid::from_mesh(mesh).unwrap().volume())
                        .sum();
                    assert!(
                        (remaining.volume() + removed - original.volume()).abs()
                            < original.volume() * 1e-5 + 1e-8,
                        "{name}: cut must conserve volume"
                    );
                    eprintln!(
                        "{name}: accepted, {} triangles, worker {:?}",
                        source.indices_u32.len() / 3,
                        result.elapsed
                    );
                }
                Err(reason) => {
                    rejected += 1;
                    assert!(
                        reason.starts_with("CSG"),
                        "{name}: actionable import/cut error: {reason}"
                    );
                    eprintln!("{name}: rejected: {reason}");
                }
            }
        }
    }
    assert!(
        accepted > 0,
        "the pickup must provide a real GLB primitive for this regression"
    );
    assert!(
        rejected > 0,
        "the pickup also exercises unsupported/open geometry"
    );
    eprintln!("pickup CSG audit: {accepted} accepted, {rejected} rejected");
}
