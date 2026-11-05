use mittens_engine::engine::{
    Universe,
    ecs::{
        World,
        component::{RenderableComponent, material::MaterialComponent},
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

#[derive(Default)]
struct Upload(u32);
impl MeshUploader for Upload {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}
impl TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
}

#[test]
fn vendor_face_override_resolves_after_gltf_loading_without_changing_body() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
        import { hot_dog_stand } from "../assets/components/hot_dog_stand.mms"
        hot_dog_stand()
        "#,
            "examples/_vendor_face_test.mms",
        )
        .unwrap();
    assert!(
        !scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some("ship_face_anime_material")),
        "the override must wait for the imported face"
    );

    let mut upload = Upload::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        scene.update(0.05, &Default::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
        // Imported nodes live beside the GLTF component under its anchor.
        let primitive = |label| {
            scene
                .world
                .all_components()
                .find(|&id| scene.world.component_label(id) == Some(label))
                .and_then(|node| {
                    scene.world.children_of(node).iter().copied().find(|&id| {
                        scene
                            .world
                            .get_component_by_id_as::<RenderableComponent>(id)
                            .is_some()
                    })
                })
        };
        let face = primitive("Face");
        let body = primitive("Body");
        if let (Some(face), Some(body)) = (face, body) {
            let face_source = scene
                .world
                .all_components()
                .find(|&id| scene.world.component_label(id) == Some("ship_face_anime_material"));
            if let Some(face_source) = face_source {
                let face_instance = scene
                    .visuals
                    .instances()
                    .iter()
                    .find(|v| v.component == face);
                let body_instance = scene
                    .visuals
                    .instances()
                    .iter()
                    .find(|v| v.component == body);
                let face_material = scene.visuals.materials.source_instance(face_source);
                if let (Some(face_instance), Some(body_instance), Some(face_material)) =
                    (face_instance, body_instance, face_material)
                {
                    assert_eq!(scene.world.parent_of(face_source), Some(face));
                    assert_eq!(face_instance.material_instance, Some(face_material));
                    assert_ne!(body_instance.material_instance, Some(face_material));
                    let material = scene
                        .world
                        .get_component_by_id_as::<MaterialComponent>(face_source)
                        .unwrap();
                    assert_eq!(material.values["shade_strength"], vec![0.28]);
                    break;
                }
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "face override never reached rendered material: face={face:?} body={body:?} overrides={:?} visuals={}",
            scene
                .world
                .all_components()
                .filter(|&id| scene.world.component_label(id) == Some("ship_face_anime_material"))
                .collect::<Vec<_>>(),
            scene.visuals.instances().len()
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
