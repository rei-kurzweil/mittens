use super::rocket_motion_tests::Uploader;
use crate::engine::Universe;
use crate::engine::ecs::component::TextComponent;
use crate::engine::ecs::system::{TextSystem, TransformSystem, bounds_system::BoundsSystem};
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use crate::utils::math::{vec3_normalize, vec3_sub};

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn prepare(scene: &mut Universe, uploader: &mut Uploader) {
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        uploader,
        &mut scene.command_queue,
    );
    scene.update(1.0 / 60.0, &Default::default());
}

#[test]
fn rooftop_avatar_picker_accepts_standing_rays_through_its_labels() {
    let path = "examples/twilight_rooftop.mms";
    let source = std::fs::read_to_string(path)
        .unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(&source, path).unwrap();
    let mut uploader = Uploader::default();
    for _ in 0..4 {
        prepare(&mut scene, &mut uploader);
    }
    let camera = named(&scene, "local_desktop_camera_anchor");
    let eye = TransformSystem::world_position(&scene.world, camera).unwrap();
    let hand = [eye[0] + 0.22, eye[1] - 0.40, eye[2] - 0.25];
    let mut lumae_click = None;
    for choice in ["mitten", "lumae", "miku", "succubus"] {
        let world = &scene.world;
        let button = named(&scene, &format!("avatar_picker_{choice}"));
        let background = world.find_component(button, "#__bg").unwrap();
        let face = world.find_component(background, "renderable").unwrap();
        let label = world.find_component(button, "text").unwrap();
        let glyphs = TextSystem::owned_text_block(world, label).unwrap();
        let glyph_model = TransformSystem::world_model(world, glyphs).unwrap();
        let label_center =
            BoundsSystem::measure_cached_renderable_subtree_bounds(world, glyphs, |_| false)
                .unwrap()
                .transformed(glyph_model)
                .center();
        let face_center = TransformSystem::world_position(world, face).unwrap();
        assert!(
            label_center[2] > face_center[2],
            "{choice}: label lies behind the face"
        );
        assert!((label_center[0] - face_center[0]).abs() < 0.03);
        assert!((label_center[1] - face_center[1]).abs() < 0.03);
        for origin in [eye, hand] {
            let hits = scene.systems.raycast.cast_against_renderables_bvh(
                world,
                &scene.systems.bvh,
                origin,
                vec3_normalize(vec3_sub(label_center, origin)),
                0.75,
                10.0,
            );
            assert_eq!(
                hits.first().map(|hit| hit.0),
                Some(face),
                "{choice}: ray from {origin:?} at {label_center:?}"
            );
        }
        if choice == "lumae" {
            lumae_click = Some((face, face_center));
        }
    }
    let (renderable, hit_point) = lumae_click.unwrap();
    scene.command_queue.push_event(
        renderable,
        EventSignal::Click {
            raycaster: named(&scene, "local_desktop_pointer"),
            renderable,
            hit_point,
            screen_pos_px: None,
        },
    );
    for _ in 0..6 {
        prepare(&mut scene, &mut uploader);
    }
    let status = named(&scene, "avatar_picker_status");
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TextComponent>(status)
            .unwrap()
            .text,
        "lumae is active"
    );
}
