use super::rocket_motion_tests::Uploader;
use crate::engine::Universe;
use crate::engine::ecs::component::{GLTFComponent, MirrorComponent, TextComponent};
use crate::engine::ecs::system::{
    TextSystem, TransformSystem, bounds_system::BoundsSystem, external_xr_frame::ExternalXrFrame,
};
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
fn click_only_panels_accept_oblique_trigger_presses_without_drag_targets() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene
        .load_mms_source_at_path(
            r#"
            import { avatar_picker_panel } from "../assets/components/panels/avatar_picker_panel.mms"
            import { mirror_panel } from "../assets/components/panels/mirror_panel.mms"
            let panel = { root = null selections = 0 }
            let picker = avatar_picker_panel({
                root_name = "picker"
                status = "Waiting"
                choices = [{ id = "miku" label = "Miku" }]
                on_select = fn(choice) {
                    panel.selections = panel.selections + 1
                    panel.root.set_status(choice + panel.selections)
                }
            })
            panel.root = picker
            T.position(-1,1.5,-2) { picker.root }
            T.position(1,1.5,-2) { mirror_panel({ name = "test_mirror" width = 1.0 height = 1.0 quality = 512 }) }
            T { InputXR.on() {
                T { CXR {} }
                XRHand.new(true,"Left","Aim") { T { Pointer {} } }
            } }
            "#,
            "examples/_click_only_panel_test.mms",
        )
        .unwrap();
    let mut uploader = Uploader::default();
    for _ in 0..4 {
        prepare(&mut scene, &mut uploader);
    }
    let mirror = named(&scene, "test_mirror");
    let text = |scene: &Universe, label: &str| {
        scene
            .world
            .get_component_by_id_as::<TextComponent>(named(scene, label))
            .unwrap()
            .text
            .clone()
    };
    let view = openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            position: openxr::Vector3f {
                x: 0.0,
                y: 1.65,
                z: 0.0,
            },
        },
        fov: openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    for (button_name, from_y, expected) in [
        ("avatar_picker_miku", -0.42_f32, "miku1"),
        ("avatar_picker_miku", 0.42_f32, "miku2"),
        ("test_mirror_toggle", -0.42_f32, "On"),
        ("test_mirror_toggle", 0.42_f32, "Off"),
    ] {
        let button = named(&scene, button_name);
        let background = scene.world.find_component(button, "#__bg").unwrap();
        let face = scene
            .world
            .find_component(background, "renderable")
            .unwrap();
        let [x, y, z] = TransformSystem::world_position(&scene.world, face).unwrap();
        let label = scene.world.find_component(button, "text").unwrap();
        let glyphs = TextSystem::owned_text_block(&scene.world, label).unwrap();
        let center =
            BoundsSystem::measure_cached_renderable_subtree_bounds(&scene.world, glyphs, |_| false)
                .unwrap()
                .transformed(TransformSystem::world_model(&scene.world, glyphs).unwrap())
                .center();
        assert!(
            (center[0] - x).abs() < 0.03 && (center[1] - y).abs() < 0.03,
            "{button_name}: label center {center:?}, face center ({x}, {y}, {z})"
        );
        let half_pitch = (-from_y).atan2(1.0) * 0.5;
        let aim = openxr::Posef {
            orientation: openxr::Quaternionf {
                x: half_pitch.sin(),
                y: 0.0,
                z: 0.0,
                w: half_pitch.cos(),
            },
            position: openxr::Vector3f {
                x,
                y: y + from_y,
                z: z + 1.0,
            },
        };
        let mut frame = ExternalXrFrame {
            views: Some([view; 2]),
            aim: [Some(aim), None],
            ..Default::default()
        };
        frame.input.trigger_pressed[0] = true;
        frame.input.trigger_down[0] = true;
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());
        assert!(!scene.systems.gesture.state().dragging);
        frame.input.trigger_pressed[0] = false;
        frame.input.trigger_down[0] = false;
        frame.input.trigger_released[0] = true;
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());

        if button_name == "avatar_picker_miku" {
            assert_eq!(text(&scene, "avatar_picker_status"), expected);
        } else {
            let engaged = from_y < 0.0;
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<MirrorComponent>(mirror)
                    .unwrap()
                    .enabled,
                engaged
            );
            assert_eq!(text(&scene, "test_mirror_toggle_label"), expected);
        }
    }
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

#[test]
fn rooftop_avatar_settings_apply_height_to_mitten_and_miku() {
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
    let original = named(&scene, "local_model");
    let slider = named(&scene, "avatar_height_slider");
    scene
        .command_queue
        .push_event(slider, EventSignal::SliderCommitted { slider, value: 1.8 });
    for _ in 0..8 {
        prepare(&mut scene, &mut uploader);
    }
    let mitten = named(&scene, "local_model");
    assert_ne!(mitten, original);
    let mitten = scene
        .world
        .get_component_by_id_as::<GLTFComponent>(mitten)
        .unwrap();
    assert!(mitten.spawned);
    assert_eq!(mitten.default_height_if_outside, Some([1.8; 3]));
    assert_eq!(
        scene.systems.avatar_settings.desired_height_meters(),
        Some(1.8)
    );

    let miku = "assets/models/local/third_party/TDA_Hatsune_Miku/tda_miku_with_bones.glb";
    if std::path::Path::new(miku).is_file() {
        let button = named(&scene, "avatar_picker_miku");
        let background = scene.world.find_component(button, "#__bg").unwrap();
        let renderable = scene
            .world
            .find_component(background, "renderable")
            .unwrap();
        scene.command_queue.push_event(
            renderable,
            EventSignal::Click {
                raycaster: named(&scene, "local_desktop_pointer"),
                renderable,
                hit_point: TransformSystem::world_position(&scene.world, renderable).unwrap(),
                screen_pos_px: None,
            },
        );
        for _ in 0..8 {
            prepare(&mut scene, &mut uploader);
        }
        let model = named(&scene, "local_model");
        let model = scene
            .world
            .get_component_by_id_as::<GLTFComponent>(model)
            .unwrap();
        assert_eq!(model.uri, miku);
        assert!((model.import_scale * 20.393234 - 1.8).abs() < 1e-4);
    }
}
