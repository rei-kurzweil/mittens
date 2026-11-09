use super::*;
use crate::engine::Universe;
use crate::engine::graphics::{MeshHandle, TextureHandle, TextureUploader};
use crate::utils::math::mat4_identity;

#[derive(Default)]
struct Uploader {
    next: u32,
    released: usize,
}
impl MeshUploader for Uploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        Ok(MeshHandle(self.next))
    }
    fn release_mesh(&mut self, _: MeshHandle) {
        self.released += 1;
    }
}
impl TextureUploader for Uploader {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
}
fn prepare(scene: &mut Universe, uploader: &mut Uploader) {
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        uploader,
        &mut scene.command_queue,
    );
    scene.update(0., &Default::default());
}
fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn fixture(extra: &str) -> Universe {
    let mut scene = Universe::new(Default::default());
    let source = format!(
        r#"
        fn specimen(label, x) {{
            let joint = T.position(0.0, 0.5, 0.0) {{ name = label + "_joint" Joint {{}} }}
            joint
            let arm = Armature {{ joint }}
            arm
            let paper = T.position(0.2, 0.0, 0.0).rotation(0.0, 0.0, 0.3).scale(1.2,0.8,1.0) {{
                name = label + "_paper"
                R.cube() {{ name = label + "_mesh" }}
                T.position(0.0, 1.0, 0.0) {{ R.cube() {{}} }}
                {extra}
            }}
            paper
            return T.position(x, 2.0, 0.0) {{
                name = label
                arm paper
                R.cube() {{ name = label + "_staff" }}
                Rigging.new(arm, paper).give_automatic_weights() {{ name = label + "_rig" }}
            }}
        }}
        specimen("a", 1.0)
        specimen("b", 5.0)
    "#
    );
    scene
        .load_mms_source_at_path(&source, "examples/_rig_test.mms")
        .unwrap();
    scene.update(0., &Default::default());
    scene
}
fn palette(scene: &Universe, mesh: ComponentId) -> Vec<TransformMatrix> {
    let r = scene
        .world
        .get_component_by_id_as::<RenderableComponent>(mesh)
        .unwrap();
    let visual = scene.visuals.instance(r.get_handle().unwrap()).unwrap();
    assert_eq!(visual.bones_count, 1);
    scene.visuals.bones_palette()
        [visual.bones_base as usize..(visual.bones_base + visual.bones_count) as usize]
        .to_vec()
}
fn near(a: TransformMatrix, b: TransformMatrix) {
    for i in 0..4 {
        for j in 0..4 {
            assert!((a[i][j] - b[i][j]).abs() < 0.0001, "{a:?} != {b:?}");
        }
    }
}

#[test]
fn authored_rig_rest_pose_root_motion_independent_instances_and_cleanup() {
    let mut scene = fixture("");
    assert_eq!(scene.systems.rigging.binding_count(), 2);
    let request = named(&scene, "a_rig");
    let mesh = named(&scene, "a_mesh");
    let other = named(&scene, "b_mesh");
    let bound = &scene.systems.rigging.bound[&request];
    assert_eq!(bound.meshes.len(), 2);
    let generated = bound.meshes[0].generated;
    let source = bound.meshes[0].source;
    assert!(
        scene
            .render_assets
            .cpu_mesh(source)
            .unwrap()
            .weights0
            .is_none()
    );
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<RenderableComponent>(named(&scene, "a_staff"))
            .unwrap()
            .renderable
            .mesh,
        source
    );
    let derived = scene.render_assets.cpu_mesh(generated).unwrap();
    for (ids, weights) in derived
        .joints0
        .as_ref()
        .unwrap()
        .iter()
        .zip(derived.weights0.as_ref().unwrap())
    {
        assert!((weights.iter().sum::<f32>() - 1.).abs() < 0.00001);
        assert!(weights.iter().all(|w| w.is_finite() && *w >= 0.));
        assert!(ids.iter().all(|i| *i == 0));
    }
    let mut uploader = Uploader::default();
    prepare(&mut scene, &mut uploader);
    for mesh in &scene.systems.rigging.bound[&request].meshes {
        near(palette(&scene, mesh.renderable)[0], mat4_identity());
    }
    near(palette(&scene, other)[0], mat4_identity());
    let uploads = uploader.next;
    for _ in 0..3 {
        scene.update(0., &Default::default());
        prepare(&mut scene, &mut uploader);
    }
    assert_eq!(
        uploader.next, uploads,
        "pose ticks must not regenerate meshes"
    );
    assert_eq!(
        scene.systems.rigging.bound[&request].meshes[0].generated,
        generated
    );

    let root = named(&scene, "a");
    let transform = TransformComponent::new()
        .with_position(12., 3., 1.)
        .with_rotation_euler(0., 0., 0.4)
        .transform;
    scene
        .systems
        .update_transform(&mut scene.world, &mut scene.visuals, root, transform);
    scene.update(0., &Default::default());
    near(palette(&scene, mesh)[0], mat4_identity());
    let joint = named(&scene, "a_joint");
    let transform = TransformComponent::new()
        .with_position(0., 0.5, 0.)
        .with_rotation_euler(0., 0., 0.5)
        .transform;
    scene
        .systems
        .update_transform(&mut scene.world, &mut scene.visuals, joint, transform);
    scene.update(0., &Default::default());
    assert_ne!(palette(&scene, mesh)[0], mat4_identity());
    near(palette(&scene, other)[0], mat4_identity());
    // Validate actual world-space skinning against the joint's bind-to-pose map.
    let binding = &scene.systems.rigging.bound[&request].meshes[0];
    let skin = scene.visuals.skin(binding.skin).unwrap();
    let mesh_frame = frame(&scene.world, mesh).unwrap();
    near(
        mat4_mul(
            TransformSystem::world_model(&scene.world, mesh_frame).unwrap(),
            palette(&scene, mesh)[0],
        ),
        mat4_mul(
            TransformSystem::world_model(&scene.world, joint).unwrap(),
            skin.inverse_bind_matrices[0],
        ),
    );

    scene
        .systems
        .remove_subtree_immediate(&mut scene.world, &mut scene.visuals, request);
    scene.update(0., &Default::default());
    assert_eq!(scene.systems.rigging.binding_count(), 1);
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<RenderableComponent>(mesh)
            .unwrap()
            .renderable
            .mesh,
        source
    );
    assert!(
        scene
            .systems
            .skinned_mesh
            .skin_id_for_renderable(mesh)
            .is_none()
    );
    prepare(&mut scene, &mut uploader);
    assert!(
        scene
            .render_assets
            .cpu_mesh(generated)
            .unwrap()
            .vertices
            .is_empty()
    );
    assert_eq!(uploader.released, 2);
}

#[test]
fn authored_rig_failure_does_not_publish_partial_mesh_tree() {
    // Reject a forbidden nested armature even after preparing valid meshes.
    let scene = fixture("Armature {}");
    assert_eq!(scene.systems.rigging.binding_count(), 0);
    for name in ["a_rig", "b_rig"] {
        assert!(
            scene
                .world
                .get_component_by_id_as::<RiggingComponent>(named(&scene, name))
                .unwrap()
                .status
                .contains("nested armature")
        );
    }
    for name in ["a_mesh", "b_mesh"] {
        let r = scene
            .world
            .get_component_by_id_as::<RenderableComponent>(named(&scene, name))
            .unwrap();
        assert!(
            scene
                .render_assets
                .cpu_mesh(r.renderable.mesh)
                .unwrap()
                .weights0
                .is_none()
        );
    }
}

#[test]
fn naive_weights_are_normalized_and_cross_disconnected_nearby_surfaces() {
    let assets = RenderAssets::new();
    let mut mesh = assets
        .cpu_mesh(crate::engine::graphics::primitives::CpuMeshHandle::CUBE)
        .unwrap()
        .clone();
    let joints = [
        [-2., 0., 0.],
        [2., 0., 0.],
        [0., 2., 0.],
        [0., -2., 0.],
        [0., 0., 2.],
    ];
    weights::assign(&mut mesh, mat4_identity(), &joints).unwrap();
    for w in mesh.weights0.as_ref().unwrap() {
        assert!((w.iter().sum::<f32>() - 1.).abs() < 0.00001);
        assert!(w.iter().all(|w| *w > 0. && w.is_finite()));
    }
    // Assignment ignores triangle connectivity: an identical isolated vertex
    // receives the same influences as its connected source vertex.
    mesh.vertices.push(mesh.vertices[0]);
    weights::assign(&mut mesh, mat4_identity(), &joints).unwrap();
    assert_eq!(
        mesh.weights0.as_ref().unwrap()[0],
        *mesh.weights0.as_ref().unwrap().last().unwrap()
    );
    let p = mesh.vertices[0].pos;
    weights::assign(&mut mesh, mat4_identity(), &[p]).unwrap();
    assert_eq!(mesh.weights0.as_ref().unwrap()[0], [1., 0., 0., 0.]);
    assert!(weights::assign(&mut mesh, mat4_identity(), &[[f32::NAN, 0., 0.]]).is_err());
}

#[test]
fn gohei_prefabs_bind_only_paper_and_animate_separate_instances() {
    let mut scene = Universe::new(Default::default());
    let time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    scene
        .systems
        .clock
        .set_driver(std::sync::Arc::new(TestClock(time.clone())));
    scene.load_mms_source_at_path(r#"
        import { gohei_squares } from "../assets/components/paper_craft/gohei_squares.mms"
        import { gohei_naive_weights } from "../assets/components/paper_craft/gohei_naive_weights.mms"
        gohei_squares("reference")
        gohei_naive_weights("one")
        T.position(2.0,0.0,0.0) { gohei_naive_weights("two") }
    "#, "examples/_gohei_test.mms").unwrap();
    scene.update(0., &Default::default());
    for (_, material) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(material.definition)
            .and_then(|d| d.custom.as_ref())
        {
            let started = std::time::Instant::now();
            while custom.compiled.get().is_none() {
                assert!(
                    started.elapsed().as_secs() < 10,
                    "shader compilation timed out"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(custom.compiled.get().unwrap().is_ok());
        }
    }
    assert_eq!(scene.systems.rigging.binding_count(), 2);
    let request = named(&scene, "one_rigging");
    assert_eq!(scene.systems.rigging.bound[&request].meshes.len(), 10);
    let mesh = scene.systems.rigging.bound[&request].meshes[0].renderable;
    let mut uploader = Uploader::default();
    prepare(&mut scene, &mut uploader);
    let initial = scene.visuals.bones_palette().to_vec();
    for tick in 1..=90 {
        time.store(tick * 16, std::sync::atomic::Ordering::Relaxed);
        scene.update(0.016, &Default::default());
    }
    assert!(
        scene.visuals.bones_palette() != initial,
        "pose demo should change palettes"
    );
    let r = scene
        .world
        .get_component_by_id_as::<RenderableComponent>(mesh)
        .unwrap();
    assert_eq!(
        scene
            .visuals
            .instance(r.get_handle().unwrap())
            .unwrap()
            .bones_count,
        5
    );
    for label in ["one_staff", "two_staff", "reference_paper"] {
        for id in nodes(&scene.world, named(&scene, label)) {
            if let Some(r) = scene
                .world
                .get_component_by_id_as::<RenderableComponent>(id)
            {
                assert!(
                    scene
                        .render_assets
                        .cpu_mesh(r.renderable.mesh)
                        .unwrap()
                        .weights0
                        .is_none()
                );
                assert!(
                    scene
                        .systems
                        .skinned_mesh
                        .skin_id_for_renderable(id)
                        .is_none()
                );
            }
        }
    }
    let root = named(&scene, "one");
    scene
        .systems
        .remove_subtree_immediate(&mut scene.world, &mut scene.visuals, root);
    scene.update(0., &Default::default());
    assert_eq!(scene.systems.rigging.binding_count(), 1);
    prepare(&mut scene, &mut uploader);
    assert_eq!(uploader.released, 10);
}

struct TestClock(std::sync::Arc<std::sync::atomic::AtomicU64>);
impl super::super::clock_system::ClockDriver for TestClock {
    fn name(&self) -> &'static str {
        "rig-test"
    }
    fn time_now_sec(&self) -> f64 {
        self.0.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.
    }
}
