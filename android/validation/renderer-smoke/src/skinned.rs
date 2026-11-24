//! `--skinned`: a compute-deformed avatar through the production renderer.
//!
//! Loads Mitten through `mitten_model` (alpha-discard anime shading), then
//! checks that the first drawn frames dispatch deformation jobs, and that
//! turning a joint dispatches again and changes the image.
use crate::{
    graphics_device,
    renderer::{SceneRenderer, view::EyeCamera},
    target::Target,
};
use ash::vk;
use mittens_engine::engine::{
    Universe,
    ecs::{World, component::{GLTFComponent, TransformComponent}},
    user_input::InputState,
};

fn prepare(universe: &mut Universe, renderer: &mut SceneRenderer) {
    universe.update(1.0 / 90.0, &InputState::default());
    universe.systems.prepare_render(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        renderer,
        &mut universe.command_queue,
    );
    universe.visuals.prepare_draw_cache();
}

fn cameras() -> [EyeCamera; 2] {
    [-0.032, 0.032].map(|x| {
        EyeCamera::from_xr(&openxr::View {
            pose: openxr::Posef {
                position: openxr::Vector3f { x, y: 0.0, z: 0.0 },
                orientation: openxr::Quaternionf { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
            },
            fov: openxr::Fovf {
                angle_left: -0.6,
                angle_right: 0.6,
                angle_down: -0.6,
                angle_up: 0.6,
            },
        })
        .unwrap()
    })
}

pub fn run() {
    // Resolve `assets/...` URIs and imports from the repository root, as the
    // launcher does from its project root.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let gpu = graphics_device::headless(false);
    let target = Target::new(gpu.clone());
    let mut renderer = SceneRenderer::with_samples(
        gpu,
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        vk::SampleCountFlags::TYPE_4,
    )
    .unwrap();
    let source = r#"
import { mitten_model } from "../assets/components/characters/mitten.mms"
AL.rgb(0.7, 0.7, 0.7)
BGC.rgba(0.1, 0.1, 0.1, 1.0)
T.position(0.0, 2.0, 2.0) { DL.color(1.0, 1.0, 1.0).intensity(1.0) }
// The same model factory as android/examples/mitten-first-person.mms,
// including the alpha-discard avatar shading used for the eye and lash layers.
T.position(0.0, -0.85, -1.4).rotation(0.0, 3.14159265, 0.0) {
    mitten_model("smoke_avatar", {}).model
}
"#;
    let mut universe = Universe::new(World::default());
    universe
        .load_mms_source_at_path(source, &repo.join("examples/_skinned_smoke.mms").to_string_lossy())
        .unwrap();
    let cameras = cameras();
    let mut drawn = None;
    let mut first_jobs = 0;
    for _ in 0..600 {
        prepare(&mut universe, &mut renderer);
        let spawned = universe.world.all_components().any(|id| {
            universe
                .world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some_and(|gltf| gltf.spawned)
        });
        let compiling = universe.visuals.materials.instances().any(|(_, record)| {
            let definition = universe.visuals.materials.definition(record.definition).unwrap();
            match definition.custom.as_ref().map(|custom| custom.compiled.get()) {
                Some(None) => true,
                Some(Some(Err(error))) => panic!("avatar shader: {error}"),
                _ => false,
            }
        });
        if spawned && !compiling {
            let stats = renderer
                .draw(0, &mut universe.visuals, &cameras)
                .unwrap_or_else(|error| panic!("skinned avatar draw: {error}"));
            first_jobs += stats.deformation_jobs;
            if stats.draws > 0 {
                drawn = Some(stats);
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let stats = drawn.expect("avatar never drew");
    let skinned = universe
        .visuals
        .instances()
        .iter()
        .filter(|instance| instance.deformed_count != 0)
        .count();
    assert!(skinned > 0, "no instance received a deformation range: {stats:?}");
    assert!(first_jobs > 0, "skinned instances drew without a dispatch: {stats:?}");
    let before = target.pixels();
    let distinct: std::collections::HashSet<_> =
        before.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
    assert!(distinct.len() > 4, "avatar render is unexpectedly uniform");

    // Unchanged poses dispatch nothing.
    prepare(&mut universe, &mut renderer);
    let idle = renderer.draw(0, &mut universe.visuals, &cameras).unwrap();
    println!("skinned avatar: first draw {stats:?}; idle frame jobs={}", idle.deformation_jobs);

    let joint = universe
        .world
        .all_components()
        .find(|&id| {
            matches!(
                universe.world.component_label(id),
                Some("J_Bip_C_Spine" | "J_Bip_C_Head")
            )
        })
        .expect("avatar spine or head joint");
    let transform = universe
        .world
        .get_component_by_id_as_mut::<TransformComponent>(joint)
        .unwrap();
    transform.transform.rotation = mittens_engine::utils::math::quat_mul(
        transform.transform.rotation,
        mittens_engine::utils::math::quat_rotation_y(0.9),
    );
    transform.transform.recompute_model();
    universe
        .systems
        .transform_changed(&mut universe.world, &mut universe.visuals, joint);
    prepare(&mut universe, &mut renderer);
    let moved = renderer.draw(0, &mut universe.visuals, &cameras).unwrap();
    assert!(moved.deformation_jobs > 0, "moving a joint dispatched nothing: {moved:?}");
    assert_ne!(before, target.pixels(), "moving a joint did not change the image");
    println!("skinned avatar: moved frame jobs={} draws={}", moved.deformation_jobs, moved.draws);
}
