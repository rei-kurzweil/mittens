use super::*;
use crate::engine::ecs::component::EmissiveComponent;
use crate::engine::ecs::{ComponentId, EventSignal, Signal};

fn labelled(universe: &Universe, name: &str) -> ComponentId {
    universe
        .world
        .all_components()
        .find(|&id| universe.world.component_label(id) == Some(name))
        .unwrap()
}

fn pump_settings_callbacks(universe: &mut Universe) {
    for _ in 0..4 {
        universe.systems.rx.begin_frame();
        universe.drain_pending_signals();
        let mut session = universe.runtime_spec_session.take().unwrap();
        let mut intents = Vec::new();
        let errors = session.service_callbacks_with_host(universe.mittens_host(&mut intents));
        universe.runtime_spec_session = Some(session);
        assert!(errors.is_empty(), "{errors:?}");
        for intent in intents {
            universe
                .command_queue
                .push_intent_now(ComponentId::default(), intent);
        }
    }
    universe.drain_pending_signals();
}

fn click(universe: &mut Universe, target: ComponentId) {
    universe.command_queue.push_event(
        target,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: target,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    pump_settings_callbacks(universe);
}

#[test]
fn info_panel_string_and_component_titles_preserve_header_ownership_and_restore() {
    use crate::engine::ecs::component::{RenderableComponent, TextComponent};
    fn below(world: &ecs::World, root: ComponentId, child: ComponentId) -> bool {
        let mut current = world.parent_of(child);
        while let Some(id) = current {
            if id == root {
                return true;
            }
            current = world.parent_of(id);
        }
        false
    }
    let mut universe = Universe::new(ecs::World::default());
    universe.load_mms_source_at_path(r#"
            import { info_panel } from "../assets/components/ui/info_panel.mms"
            import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
            info_panel({
                root_name = "string_panel" width_gu = 32.0 unit_scale = 0.08
                title = "Plain title" content = T { Text { "Body" } }
            })
            let header = T {
                name = "custom_header"
                Grabbable.parent()
                Raycastable.enabled()
                Style { display("flex") flex_direction("row") width(100%) height(3.5) }
                T { Text { name = "custom_caption" "Custom title" } }
                R.square() { name = "custom_header_surface" }
            }
            graphics_settings_panel({ root_name = "custom_panel" title = header })
        "#, "examples/graphics-settings.mms").unwrap();
    let string_panel = labelled(&universe, "string_panel");
    let text_wrapper = universe
        .world
        .find_component(string_panel, "#info_panel_title")
        .unwrap();
    assert!(universe.world.all_components().any(|id| {
        below(&universe.world, text_wrapper, id)
            && universe
                .world
                .get_component_by_id_as::<TextComponent>(id)
                .is_some_and(|text| text.text == "Plain title")
    }));
    let panel = labelled(&universe, "custom_panel");
    let header = labelled(&universe, "custom_header");
    let caption = labelled(&universe, "custom_caption");
    assert!(
        universe
            .world
            .find_component(panel, "#info_panel_title")
            .is_none(),
        "a custom bar must not be nested in the default title label"
    );
    assert!(below(&universe.world, panel, header));
    assert!(below(&universe.world, header, caption));
    let header_hit = labelled(&universe, "custom_header_surface");
    assert!(
        crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
            &universe.world,
            header_hit
        )
        .is_none(),
        "panel controls must not become editor selection targets"
    );
    assert_eq!(
        crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit(
            &universe.world,
            header_hit
        ),
        Some(panel),
        "the custom header's parent grab targets the whole panel"
    );
    let slider = labelled(&universe, "graphics_mirrors_quality_slider");
    let slider_hit = universe
        .world
        .all_components()
        .find(|id| {
            below(&universe.world, slider, *id)
                && universe
                    .world
                    .get_component_by_id_as::<RenderableComponent>(*id)
                    .is_some()
        })
        .unwrap();
    assert_eq!(
        crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit(
            &universe.world,
            slider_hit
        ),
        None,
        "header-only grab must not cover slider geometry"
    );
    let toggle = universe
        .world
        .find_component(header, "#accordion_toggle")
        .unwrap();
    // A grip grab reparents this root away from the factory's layout slot.
    // Restore listeners and selection exclusion must travel with it.
    let original_parent = universe.world.parent_of(panel);
    let held_parent = universe
        .world
        .add_component(crate::engine::ecs::component::TransformComponent::new());
    universe.world.set_parent(panel, Some(held_parent)).unwrap();
    assert!(
        crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
            &universe.world,
            header_hit
        )
        .is_none()
    );
    for _ in 0..2 {
        click(&mut universe, toggle);
        assert!(
            universe
                .world
                .find_component(panel, "#accordion_body")
                .is_none()
        );
        click(&mut universe, toggle);
        assert!(
            universe
                .world
                .find_component(panel, "#accordion_body")
                .is_some()
        );
        assert_eq!(labelled(&universe, "custom_header"), header);
        assert_eq!(labelled(&universe, "custom_caption"), caption);
    }
    universe.world.set_parent(panel, original_parent).unwrap();
    let content = labelled(&universe, "graphics_settings_content");
    let rows: Vec<_> = universe
        .world
        .children_of(content)
        .iter()
        .copied()
        .filter(|id| {
            universe
                .world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(*id)
                .is_some()
        })
        .collect();
    assert_eq!(rows.len(), 9);
    assert_eq!(
        universe.world.component_label(rows[0]),
        Some("graphics_bloom_heading")
    );
    assert_eq!(
        universe.world.component_label(rows[3]),
        Some("graphics_mirrors_heading")
    );
    assert_eq!(
        universe.world.component_label(rows[6]),
        Some("graphics_secondary_motion_heading")
    );
    for (row, label) in [
        (rows[1], "Toggle bloom"),
        (rows[2], "Pyramid levels"),
        (rows[4], "Toggle mirrors"),
        (rows[5], "Mirror resolution"),
        (rows[7], "Toggle secondary motion"),
    ] {
        assert!(
            universe
                .world
                .all_components()
                .any(|id| below(&universe.world, row, id)
                    && universe
                        .world
                        .get_component_by_id_as::<TextComponent>(id)
                        .is_some_and(|text| text.text == label)),
            "missing row label {label}"
        );
    }
    for id in universe.world.all_components().filter(|id| {
        below(&universe.world, panel, *id)
            && universe
                .world
                .get_component_by_id_as::<RenderableComponent>(*id)
                .is_some()
    }) {
        assert!(
            crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
                &universe.world,
                id
            )
            .is_none(),
            "restored graphics panel renderables must not be selectable"
        );
    }
}

#[test]
fn graphics_settings_toggle_updates_backend_and_restores_authoritative_values() {
    use crate::engine::ecs::component::{SliderComponent, TextComponent, ToggleComponent};
    let mut universe = Universe::new(ecs::World::default());
    universe
        .systems
        .toggle
        .install_handlers(&mut universe.systems.rx);
    universe.load_mms_source_at_path(r#"
            import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
            graphics_settings_panel({ root_name = "test_graphics_settings" })
        "#, "examples/graphics-settings.mms").unwrap();
    assert!(universe.systems.graphics_settings.mirrors_enabled());
    let control = labelled(&universe, "graphics_mirrors_toggle_control");
    click(&mut universe, control);
    assert!(!universe.systems.graphics_settings.mirrors_enabled());
    assert!(!universe.visuals.mirror_captures_enabled());
    let motion_control = labelled(&universe, "graphics_secondary_motion_toggle_control");
    click(&mut universe, motion_control);
    assert!(
        !universe
            .systems
            .graphics_settings
            .secondary_motion_enabled()
    );
    let slider = labelled(&universe, "graphics_mirrors_quality_slider");
    universe.command_queue.push_intent_now(
        slider,
        ecs::IntentValue::SliderSet {
            component_id: slider,
            value: 5.0,
            emit_changed: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert_eq!(
        universe.systems.graphics_settings.mirror_quality_override(),
        Some(1440)
    );
    assert_eq!(universe.visuals.mirror_quality_override(), Some(1440));
    let panel = labelled(&universe, "test_graphics_settings");
    let chevron = labelled(&universe, "accordion_toggle");
    for _ in 0..2 {
        click(&mut universe, chevron);
        assert!(
            universe
                .world
                .find_component(panel, "#accordion_body")
                .is_none()
        );
        click(&mut universe, chevron);
        assert!(
            universe
                .world
                .find_component(panel, "#accordion_body")
                .is_some()
        );
        assert!(
            !universe
                .world
                .get_component_by_id_as::<ToggleComponent>(labelled(
                    &universe,
                    "graphics_mirrors_toggle"
                ))
                .unwrap()
                .value()
        );
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<SliderComponent>(labelled(
                    &universe,
                    "graphics_mirrors_quality_slider"
                ))
                .unwrap()
                .value(),
            5.0
        );
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<TextComponent>(labelled(
                    &universe,
                    "graphics_mirrors_quality_readout"
                ))
                .unwrap()
                .text,
            "1440"
        );
        assert!(
            !universe
                .world
                .get_component_by_id_as::<ToggleComponent>(labelled(
                    &universe,
                    "graphics_secondary_motion_toggle"
                ))
                .unwrap()
                .value()
        );
    }
    // External changes synchronize the native control without emitting a
    // ToggleChanged callback or becoming a second settings authority.
    universe
        .systems
        .graphics_settings
        .set_mirrors_enabled(true, &mut universe.command_queue);
    pump_settings_callbacks(&mut universe);
    assert!(universe.visuals.mirror_captures_enabled());
    assert!(
        universe
            .world
            .get_component_by_id_as::<ToggleComponent>(labelled(
                &universe,
                "graphics_mirrors_toggle"
            ))
            .unwrap()
            .value()
    );
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<TextComponent>(labelled(
                &universe,
                "graphics_mirrors_enabled_readout"
            ))
            .unwrap()
            .text,
        "On"
    );
    universe
        .systems
        .graphics_settings
        .set_mirror_quality(700, &mut universe.command_queue);
    universe
        .systems
        .graphics_settings
        .set_secondary_motion_enabled(true, &mut universe.command_queue);
    pump_settings_callbacks(&mut universe);
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<TextComponent>(labelled(
                &universe,
                "graphics_mirrors_quality_readout"
            ))
            .unwrap()
            .text,
        "700"
    );
    assert_eq!(
        universe.systems.graphics_settings.mirror_quality_override(),
        Some(700),
        "silent slider sync must not snap an external setting to a preset"
    );
    assert!(
        universe
            .world
            .get_component_by_id_as::<ToggleComponent>(labelled(
                &universe,
                "graphics_secondary_motion_toggle"
            ))
            .unwrap()
            .value()
    );
    // Settings changed while the body is absent must be read on restore.
    click(&mut universe, chevron);
    universe
        .systems
        .graphics_settings
        .reset_mirror_quality(&mut universe.command_queue);
    universe
        .systems
        .graphics_settings
        .set_secondary_motion_enabled(false, &mut universe.command_queue);
    pump_settings_callbacks(&mut universe);
    click(&mut universe, chevron);
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<TextComponent>(labelled(
                &universe,
                "graphics_mirrors_quality_readout"
            ))
            .unwrap()
            .text,
        "Authored"
    );
    assert!(
        !universe
            .world
            .get_component_by_id_as::<ToggleComponent>(labelled(
                &universe,
                "graphics_secondary_motion_toggle"
            ))
            .unwrap()
            .value()
    );
}

#[test]
fn xr_settings_host_validates_readback_and_resets_with_session() {
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r#"
            XrSettings.floor.set_correction_meters(0.15)
            Text { name = "xr_readback" "" + XrSettings.floor.correction_meters() }
        "#,
            "examples/xr-settings.mms",
        )
        .unwrap();
    assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
    for source in [
        "XrSettings.floor.set_correction_meters(true)",
        "XrSettings.floor.set_correction_meters()",
        "XrSettings.floor.set_correction_meters(0.1, 0.2)",
        "XrSettings.floor.set_correction_meters(0.51)",
        "XrSettings.floor.set_correction_meters(-0.51)",
        "XrSettings.floor.correction_meters(0.1)",
    ] {
        assert!(
            universe
                .load_mms_source_at_path(source, "examples/xr-settings.mms")
                .is_err(),
            "{source}"
        );
        assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
    }
    let text = universe
        .world
        .get_component_by_id_as::<ecs::component::TextComponent>(labelled(&universe, "xr_readback"))
        .unwrap();
    assert!((text.text.parse::<f32>().unwrap() - 0.15).abs() < 1e-6);
    universe
        .load_mms_source_at_path("Transform {}", "examples/xr-settings.mms")
        .unwrap();
    assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
    universe
        .systems
        .xr_settings
        .set_floor_correction_meters(-0.5, &mut universe.command_queue)
        .unwrap();
    universe.close_mms_session();
    assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
}

#[test]
fn xr_settings_panel_slider_reset_external_updates_and_restore() {
    use ecs::component::{SliderComponent, TextComponent};
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r##"
            import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
            T { name = "settings_mount" }
            let mount = query("#settings_mount")
            mount.attach(xr_settings_panel({ root_name = "test_xr_settings" }))
        "##,
            "examples/xr-settings.mms",
        )
        .unwrap();
    let slider = labelled(&universe, "xr_floor_correction_slider");
    universe.command_queue.push_intent_now(
        slider,
        ecs::IntentValue::SliderSet {
            component_id: slider,
            value: 0.15,
            emit_changed: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
    let readout = |universe: &Universe| {
        universe
            .world
            .get_component_by_id_as::<TextComponent>(labelled(
                universe,
                "xr_floor_correction_readout",
            ))
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(readout(&universe), "+15.0 cm");
    let panel = labelled(&universe, "test_xr_settings");
    let chevron = labelled(&universe, "accordion_toggle");
    for correction in [-0.2, 0.5, -0.5] {
        click(&mut universe, chevron);
        assert!(
            universe
                .world
                .find_component(panel, "#accordion_body")
                .is_none()
        );
        universe
            .systems
            .xr_settings
            .set_floor_correction_meters(correction, &mut universe.command_queue)
            .unwrap();
        pump_settings_callbacks(&mut universe);
        click(&mut universe, chevron);
        let slider = labelled(&universe, "xr_floor_correction_slider");
        assert!(
            (universe
                .world
                .get_component_by_id_as::<SliderComponent>(slider)
                .unwrap()
                .value()
                - correction as f32)
                .abs()
                < 1e-6
        );
    }
    universe
        .systems
        .xr_settings
        .set_floor_correction_meters(-0.125, &mut universe.command_queue)
        .unwrap();
    pump_settings_callbacks(&mut universe);
    assert_eq!(readout(&universe), "-12.5 cm");
    let reset = labelled(&universe, "xr_floor_correction_reset");
    click(&mut universe, reset);
    assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
    assert_eq!(readout(&universe), "0.0 cm");
}

#[test]
fn settings_panels_in_grabbable_wrappers_allow_xr_slider_drags() {
    use ecs::component::{
        ControllerHand, ControllerPoseKind, ControllerXRComponent, PointerComponent,
        SliderComponent, TextComponent, TransformComponent,
    };
    use ecs::system::TransformSystem;
    use ecs::system::grabbable_system::grabbable_owner_for_hit;
    use ecs::system::pointer_system::PointerActivations;

    fn pointer_frame(
        universe: &mut Universe,
        driver: ComponentId,
        origin: [f32; 3],
        activations: PointerActivations,
    ) {
        universe
            .world
            .get_component_by_id_as_mut::<TransformComponent>(driver)
            .unwrap()
            .set_position(&mut universe.command_queue, origin[0], origin[1], origin[2]);
        universe.drain_pending_signals();
        universe.systems.rx.begin_frame();
        universe.systems.gesture.begin_frame();
        universe.systems.bvh.flush_pending(&universe.world);
        universe.systems.raycast.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &InputState::default(),
            &mut universe.systems.rx,
            &universe.systems.bvh,
            &activations,
            &universe.systems.pointer,
            1.0 / 90.0,
        );
        universe.drain_pending_signals();
        universe.systems.gesture.tick_with_rx(
            &mut universe.world,
            &InputState::default(),
            &activations,
            &universe.systems.pointer,
            &universe.systems.raycast,
            &mut universe.systems.rx,
        );
        pump_settings_callbacks(universe);
    }

    for (factory, slider_name, readout_name, expected_text) in [
        (
            "graphics_settings_panel",
            "graphics_mirrors_quality_slider",
            "graphics_mirrors_quality_readout",
            "1440",
        ),
        (
            "xr_settings_panel",
            "xr_floor_correction_slider",
            "xr_floor_correction_readout",
            "+30.0 cm",
        ),
    ] {
        let mut universe = Universe::new(ecs::World::default());
        let source = format!(
            r#"
                import {{ {factory} }} from "../assets/components/panels/{factory}.mms"
                T.position(-2.6, 2.3, -1.5) {{
                    name = "settings_mount"
                    Grabbable {{}}
                    {factory}({{ root_name = "settings_panel" }})
                }}
                "#
        );
        universe
            .load_mms_source_at_path(&source, "examples/_settings_drag_test.mms")
            .unwrap();
        universe
            .systems
            .slider
            .install_handlers(&mut universe.systems.rx);
        universe
            .systems
            .gesture
            .install_handlers(&mut universe.systems.rx);
        let controller = universe.world.add_component(ControllerXRComponent::new(
            true,
            ControllerHand::Left,
            ControllerPoseKind::Aim,
        ));
        let driver = universe.world.add_component(TransformComponent::new());
        let pointer = universe.world.add_component(PointerComponent::new());
        universe.world.add_child(controller, driver).unwrap();
        universe.world.add_child(driver, pointer).unwrap();
        universe
            .world
            .init_component_tree(driver, &mut universe.command_queue);
        universe.drain_pending_signals();
        let panel = labelled(&universe, "settings_panel");
        let mount = labelled(&universe, "settings_mount");
        let panel_model = TransformSystem::world_model(&universe.world, panel).unwrap();

        for restored in [false, true] {
            if restored {
                let toggle = labelled(&universe, "accordion_toggle");
                click(&mut universe, toggle);
                click(&mut universe, toggle);
            }
            universe
                .systems
                .layout
                .tick(&mut universe.world, &mut universe.command_queue);
            universe.drain_pending_signals();
            let slider = labelled(&universe, slider_name);
            let config = universe
                .world
                .get_component_by_id_as::<SliderComponent>(slider)
                .unwrap();
            let width = config.width();
            let track = universe
                .world
                .find_component(config.track_mount.unwrap(), "Renderable")
                .unwrap();
            let slider_model = TransformSystem::world_model(&universe.world, slider).unwrap();
            let ray_origin = |fraction: f32| {
                let point = crate::utils::math::mat4_mul_vec4(
                    slider_model,
                    [(fraction - 0.5) * width, 0.0, 0.0, 1.0],
                );
                [point[0], point[1], point[2] + 1.0]
            };
            pointer_frame(
                &mut universe,
                driver,
                ray_origin(0.2),
                PointerActivations {
                    pressed: vec![pointer],
                    down: vec![pointer],
                    ..Default::default()
                },
            );
            assert_eq!(
                universe.systems.gesture.state().drag_renderable,
                Some(track),
                "{factory}: XR trigger must acquire the track (restored={restored})"
            );
            pointer_frame(
                &mut universe,
                driver,
                ray_origin(0.8),
                PointerActivations {
                    down: vec![pointer],
                    ..Default::default()
                },
            );
            assert_eq!(
                universe
                    .world
                    .get_component_by_id_as::<TextComponent>(labelled(&universe, readout_name))
                    .unwrap()
                    .text,
                expected_text,
                "{factory}: moving the controller must apply the slider value"
            );
            if factory == "xr_settings_panel" {
                assert!(
                    (universe.systems.xr_settings.floor_correction_meters() - 0.3).abs() < 1e-5
                );
            } else {
                assert_eq!(
                    universe.systems.graphics_settings.mirror_quality_override(),
                    Some(1440)
                );
            }
            pointer_frame(
                &mut universe,
                driver,
                ray_origin(0.8),
                PointerActivations {
                    released: vec![pointer],
                    ..Default::default()
                },
            );
            assert!(!universe.systems.gesture.state().dragging);
            assert_eq!(
                TransformSystem::world_model(&universe.world, panel).unwrap(),
                panel_model,
                "slider input must not move the panel"
            );
            let title = universe.world.find_component(panel, "#title_bar").unwrap();
            let title_hit = universe.world.find_component(title, "Renderable").unwrap();
            assert_eq!(
                grabbable_owner_for_hit(&universe.world, title_hit),
                Some(mount)
            );
        }
    }
}

#[test]
fn xr_settings_multiple_panels_share_backend_after_one_is_removed() {
    use ecs::component::SliderComponent;
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r#"
            import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
            xr_settings_panel({ root_name = "xr_panel_a" })
            xr_settings_panel({ root_name = "xr_panel_b" })
        "#,
            "examples/xr-settings.mms",
        )
        .unwrap();
    let panel_a = labelled(&universe, "xr_panel_a");
    let panel_b = labelled(&universe, "xr_panel_b");
    let slider_a = universe
        .world
        .find_component(panel_a, "#xr_floor_correction_slider")
        .unwrap();
    universe.command_queue.push_intent_now(
        slider_a,
        ecs::IntentValue::SliderSet {
            component_id: slider_a,
            value: -0.15,
            emit_changed: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    let slider_b = universe
        .world
        .find_component(panel_b, "#xr_floor_correction_slider")
        .unwrap();
    assert!(
        (universe
            .world
            .get_component_by_id_as::<SliderComponent>(slider_b)
            .unwrap()
            .value()
            + 0.15)
            .abs()
            < 1e-6
    );
    universe.command_queue.push_intent_now(
        panel_a,
        ecs::IntentValue::RemoveSubtree {
            component_id: panel_a,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert!(universe.world.get_component_record(panel_a).is_none());
    assert!((universe.systems.xr_settings.floor_correction_meters() + 0.15).abs() < 1e-6);
    universe
        .systems
        .xr_settings
        .set_floor_correction_meters(0.25, &mut universe.command_queue)
        .unwrap();
    pump_settings_callbacks(&mut universe);
    assert!(
        (universe
            .world
            .get_component_by_id_as::<SliderComponent>(slider_b)
            .unwrap()
            .value()
            - 0.25)
            .abs()
            < 1e-6
    );
}

#[test]
fn xr_settings_example_loads_matching_graphics_scene_fixture() {
    use ecs::component::{CameraXRComponent, GLTFComponent, MirrorComponent};
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            include_str!("../../examples/xr-settings.mms"),
            "examples/xr-settings.mms",
        )
        .unwrap();
    for name in [
        "xr_settings",
        "xr_settings_title_bar",
        "xr_settings_mirror",
        "studio_floor",
        "mitten_xr_driver",
        "xr_floor_correction_slider",
    ] {
        labelled(&universe, name);
    }
    assert!(
        !universe
            .world
            .all_components()
            .any(|id| universe.world.component_label(id) == Some("graphics_settings"))
    );
    assert!(universe.world.all_components().any(|id| {
        universe
            .world
            .get_component_by_id_as::<CameraXRComponent>(id)
            .is_some()
    }));
    assert!(universe.world.all_components().any(|id| {
        universe
            .world
            .get_component_by_id_as::<MirrorComponent>(id)
            .is_some()
    }));
    assert!(universe.world.all_components().any(|id| {
        universe
            .world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| gltf.uri == "assets/models/mitten.glb")
    }));
}

#[test]
fn graphics_bloom_panel_controls_live_layers_without_discarding_authored_graph() {
    use ecs::component::SliderComponent;
    let mut universe = Universe::new(ecs::World::default());
    universe.load_mms_source_at_path(r#"
            import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
            RenderGraph { EmissivePass {} Bloom { intensity(0.7) } }
            graphics_settings_panel({ root_name = "bloom_settings" })
        "#, "examples/_bloom_settings_test.mms").unwrap();
    let toggle = labelled(&universe, "graphics_bloom_toggle");
    universe.command_queue.push_event(
        toggle,
        EventSignal::ToggleChanged {
            toggle,
            value: false,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert!(!universe.systems.graphics_settings.bloom_enabled());
    assert_eq!(
        universe
            .visuals
            .effective_post_processing()
            .bloom
            .unwrap()
            .intensity,
        0.0
    );
    let slider = labelled(&universe, "graphics_bloom_layers_slider");
    universe.command_queue.push_intent_now(
        slider,
        ecs::IntentValue::SliderSet {
            component_id: slider,
            value: 4.0,
            emit_changed: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert_eq!(universe.systems.graphics_settings.bloom_layers(), 4);
    assert_eq!(
        universe
            .visuals
            .effective_post_processing()
            .bloom
            .unwrap()
            .layers,
        4
    );
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<SliderComponent>(slider)
            .unwrap()
            .value(),
        4.0
    );
    universe.command_queue.push_event(
        toggle,
        EventSignal::ToggleChanged {
            toggle,
            value: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert_eq!(
        universe
            .visuals
            .effective_post_processing()
            .bloom
            .unwrap()
            .intensity,
        0.7
    );
    assert!(
        universe
            .systems
            .graphics_settings
            .set_bloom_layers(0, &mut universe.command_queue)
            .is_err()
    );
    assert_eq!(universe.systems.graphics_settings.bloom_layers(), 4);
}

#[test]
fn graphics_settings_host_reads_applied_state_and_resets_on_scene_replacement() {
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r#"
            let mirrors = GraphicsSettings.mirrors
            mirrors.toggle(false)
            Text { name = "settings_readback" "" + mirrors.enabled() }
        "#,
            "examples/graphics-settings.mms",
        )
        .unwrap();
    assert!(!universe.systems.graphics_settings.mirrors_enabled());
    assert!(!universe.visuals.mirror_captures_enabled());
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(labelled(
                &universe,
                "settings_readback"
            ))
            .unwrap()
            .text,
        "false"
    );
    universe
        .load_mms_source_at_path("Transform {}", "examples/graphics-settings.mms")
        .unwrap();
    assert!(universe.systems.graphics_settings.mirrors_enabled());
    universe
        .systems
        .graphics_settings
        .set_mirrors_enabled(false, &mut universe.command_queue);
    universe.close_mms_session();
    assert!(universe.systems.graphics_settings.mirrors_enabled());
    assert!(universe.visuals.mirror_captures_enabled());
}

#[test]
fn graphics_settings_calls_reject_invalid_arguments_without_edit_mode() {
    let mut universe = Universe::new(ecs::World::default());
    for source in [
        "GraphicsSettings.mirrors.toggle(1)",
        "GraphicsSettings.mirrors.toggle()",
        "GraphicsSettings.mirrors.toggle(false, true)",
        "GraphicsSettings.mirrors.enabled(false)",
        "GraphicsSettings.mirrors.quality(512.5)",
        "GraphicsSettings.mirrors.quality(true)",
        "GraphicsSettings.mirrors.quality(2147483648)",
        "GraphicsSettings.mirrors.quality()",
        "GraphicsSettings.mirrors.quality_override(512)",
        "GraphicsSettings.mirrors.reset_quality(512)",
        "GraphicsSettings.secondary_motion.toggle(0)",
        "GraphicsSettings.secondary_motion.toggle()",
        "GraphicsSettings.secondary_motion.enabled(false)",
    ] {
        assert!(
            universe
                .load_mms_source_at_path(source, "examples/graphics-settings.mms")
                .is_err(),
            "accepted {source}"
        );
        assert!(universe.systems.graphics_settings.mirrors_enabled());
    }
    universe
        .load_mms_source_at_path(
            "GraphicsSettings.mirrors.toggle(false)",
            "examples/graphics-settings.mms",
        )
        .unwrap();
    assert!(!universe.visuals.mirror_captures_enabled());
    assert!(!universe.world.all_components().any(|id| {
        universe
            .world
            .get_component_by_id_as::<ecs::component::EditorComponent>(id)
            .is_some()
    }));
}

#[test]
fn graphics_settings_quality_and_motion_host_readback_clamp_reset_and_scene_defaults() {
    use crate::engine::ecs::component::TextComponent;
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r#"
            let mirrors = GraphicsSettings.mirrors
            mirrors.quality(-10)
            Text { name = "minimum" "" + mirrors.quality_override() }
            mirrors.quality(10000)
            Text { name = "maximum" "" + mirrors.quality_override() }
            mirrors.reset_quality()
            if mirrors.quality_override() == null { Text { name = "reset" "Authored" } }
            mirrors.quality(1024)
            GraphicsSettings.secondary_motion.toggle(false)
            Text { name = "motion" "" + GraphicsSettings.secondary_motion.enabled() }
        "#,
            "examples/graphics-settings.mms",
        )
        .unwrap();
    for (name, expected) in [
        ("minimum", "64"),
        ("maximum", "2048"),
        ("reset", "Authored"),
        ("motion", "false"),
    ] {
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<TextComponent>(labelled(&universe, name))
                .unwrap()
                .text,
            expected
        );
    }
    assert_eq!(
        universe.systems.graphics_settings.mirror_quality_override(),
        Some(1024)
    );
    assert_eq!(universe.visuals.mirror_quality_override(), Some(1024));
    assert!(
        !universe
            .systems
            .graphics_settings
            .secondary_motion_enabled()
    );
    // A failed replacement restores the previous settings rather than defaults.
    assert!(
        universe
            .load_mms_source_at_path(
                "GraphicsSettings.mirrors.quality(false)",
                "examples/graphics-settings.mms"
            )
            .is_err()
    );
    assert_eq!(
        universe.systems.graphics_settings.mirror_quality_override(),
        Some(1024)
    );
    assert!(
        !universe
            .systems
            .graphics_settings
            .secondary_motion_enabled()
    );
    universe
        .load_mms_source_at_path("Transform {}", "examples/graphics-settings.mms")
        .unwrap();
    assert_eq!(
        universe.systems.graphics_settings.mirror_quality_override(),
        None
    );
    assert_eq!(universe.visuals.mirror_quality_override(), None);
    assert!(
        universe
            .systems
            .graphics_settings
            .secondary_motion_enabled()
    );
    universe
        .systems
        .graphics_settings
        .set_mirror_quality(2048, &mut universe.command_queue);
    universe
        .systems
        .graphics_settings
        .set_secondary_motion_enabled(false, &mut universe.command_queue);
    universe.close_mms_session();
    assert_eq!(
        universe.systems.graphics_settings.mirror_quality_override(),
        None
    );
    assert!(
        universe
            .systems
            .graphics_settings
            .secondary_motion_enabled()
    );
}

#[test]
fn mms_facade_installs_replaces_and_closes_the_retained_session() {
    let mut universe = Universe::new(ecs::World::default());
    let source = r#"Transform { name = "universe-mms-session" }"#;

    let output = universe
        .load_mms_source_at_path(source, "Cargo.toml")
        .unwrap();
    assert!(output.errors.is_empty());
    assert!(universe.runtime_spec_session.is_some());
    assert!(
        universe
            .world
            .all_components()
            .any(|id| { universe.world.component_label(id) == Some("universe-mms-session") })
    );

    universe
        .load_mms_source_at_path(
            r#"Transform { name = "replacement-mms-session" }"#,
            "Cargo.toml",
        )
        .unwrap();
    assert!(universe.runtime_spec_session.is_some());

    universe.close_mms_session();
    assert!(universe.runtime_spec_session.is_none());
}

#[test]
fn mms_facade_services_retained_callbacks_with_a_fresh_host_lease() {
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r#"
                    let glow = Emissive.on() { name = "facade-glow" intensity(1.0) }
                    let root = Transform { name = "facade-root" glow }
                    on(root, "Click", fn(event) { glow.set_intensity(2.0) })
                    root
                "#,
            "Cargo.toml",
        )
        .unwrap();
    let root = universe
        .world
        .all_components()
        .find(|&id| universe.world.component_label(id) == Some("facade-root"))
        .unwrap();
    let glow = universe
        .world
        .all_components()
        .find(|&id| universe.world.component_label(id) == Some("facade-glow"))
        .unwrap();

    universe.systems.rx.dispatch_event_handlers(
        &mut universe.world,
        &Signal::event(
            root,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: root,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        ),
    );
    universe.service_runtime_spec_callbacks();
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<EmissiveComponent>(glow)
            .unwrap()
            .intensity,
        2.0
    );
}
