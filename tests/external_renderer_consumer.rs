//! Verify the public scene/upload boundary available to an application-owned
//! renderer, without initializing the built-in Vulkan or OpenXR renderer.
use mittens_engine::engine::{
    Universe,
    ecs::World,
    graphics::visual_world::RenderOp,
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
    user_input::InputState,
};

#[derive(Default)]
struct ApplicationUploader {
    meshes: Vec<CpuMesh>,
}

impl MeshUploader for ApplicationUploader {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        let handle = MeshHandle(1000 + self.meshes.len() as u32);
        self.meshes.push(mesh.clone());
        Ok(handle)
    }
}

impl TextureUploader for ApplicationUploader {
    fn upload_texture_rgba8(
        &mut self,
        _rgba: &[u8],
        _width: u32,
        _height: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Err("this geometry-only consumer does not implement texture uploads".into())
    }

    fn upload_texture_bc7(
        &mut self,
        _blocks: &[u8],
        _width: u32,
        _height: u32,
        _srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Err("this consumer does not support BC7".into())
    }
}

#[test]
fn application_renderer_receives_mms_geometry_and_shared_draw_stream() {
    let mut universe = Universe::new(World::default());
    universe
        .load_mms_source_at_path(
            r#"
T.position(0.0, 0.0, -2.0) { R.cube() { C.rgba(1.0, 0.0, 0.0, 1.0) } }
T.position(1.0, 0.0, -3.0) { R.cube() { C.rgba(0.0, 0.0, 1.0, 1.0) } }
"#,
            "tests/external-renderer-scene.mms",
        )
        .expect("load engine-owned MMS scene");
    let mut uploader = ApplicationUploader::default();

    // The host calls these public APIs instead of Universe::render(), which
    // currently selects the built-in Vulkano renderer and XR frame driver.
    for _ in 0..3 {
        universe.update(1.0 / 90.0, &InputState::default());
        universe.systems.prepare_render(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.render_assets,
            &mut uploader,
            &mut universe.command_queue,
        );
        universe.visuals.prepare_draw_cache();
    }

    assert_eq!(uploader.meshes.len(), 1, "shared cube uploads once");
    let cube = &uploader.meshes[0];
    assert!(!cube.vertices.is_empty());
    assert!(!cube.indices_u32.is_empty());
    assert!(
        cube.indices_u32
            .iter()
            .all(|&index| (index as usize) < cube.vertices.len())
    );

    let instances = universe.visuals.instances();
    assert_eq!(instances.len(), 2);
    assert!(
        instances
            .iter()
            .all(|instance| instance.renderable.mesh == MeshHandle(1000))
    );
    for (color, position) in [
        ([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, -2.0]),
        ([0.0, 0.0, 1.0, 1.0], [1.0, 0.0, -3.0]),
    ] {
        let instance = instances
            .iter()
            .find(|instance| instance.color == color)
            .unwrap();
        assert_eq!(&instance.transform.matrix_world[3][..3], &position);
    }

    let (operations, indices) = universe.visuals.opaque_stream();
    let mut drawn = Vec::new();
    for operation in operations {
        if let RenderOp::DrawBatch(batch) = operation {
            assert_eq!(batch.mesh, MeshHandle(1000));
            drawn.extend_from_slice(&indices[batch.start..batch.start + batch.count]);
        }
    }
    drawn.sort_unstable();
    assert_eq!(
        drawn,
        [0, 1],
        "consumer sees both cubes through the engine draw stream"
    );
}
