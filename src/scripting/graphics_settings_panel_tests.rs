use crate::engine::{
    Universe,
    ecs::system::graphics_settings_system::{
        XrFoveationFeedback, XrFoveationQuality, XrFoveationStatus,
    },
    ecs::{
        ComponentId, EventSignal, World,
        component::{RaycastableComponent, SliderComponent, TextComponent, ToggleComponent},
    },
};

fn named(scene: &Universe, root: Option<ComponentId>, name: &str) -> ComponentId {
    let mut nodes = root
        .map(|id| vec![id])
        .unwrap_or_else(|| scene.world.all_components().collect());
    while let Some(id) = nodes.pop() {
        if scene.world.component_label(id) == Some(name) {
            return id;
        }
        nodes.extend(scene.world.children_of(id));
    }
    panic!("missing {name}");
}
fn update(scene: &mut Universe) {
    for _ in 0..3 {
        scene.update(0.016, &Default::default());
    }
}
fn click(scene: &mut Universe, id: ComponentId) {
    scene.systems.rx.push_event(
        id,
        EventSignal::Click {
            raycaster: id,
            renderable: id,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    update(scene);
}
fn readout(scene: &Universe, panel: ComponentId) -> &str {
    let id = named(scene, Some(panel), "graphics_foveation_quality_readout");
    &scene
        .world
        .get_component_by_id_as::<TextComponent>(id)
        .unwrap()
        .text
}

#[test]
fn foveation_builtin_and_shared_panels_sync_through_minimize_restore() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
        GraphicsSettings.foveation.quality("low")
        graphics_settings_panel({root_name="foveation_panel_a"})
        graphics_settings_panel({root_name="foveation_panel_b"})
    "#, "examples/graphics-settings.mms").unwrap();
    update(&mut scene);
    let a = named(&scene, None, "foveation_panel_a");
    let b = named(&scene, None, "foveation_panel_b");
    assert_eq!(readout(&scene, a), "Low");
    assert_eq!(readout(&scene, b), "Low");
    let slider = named(&scene, Some(a), "graphics_foveation_quality_slider");
    scene
        .systems
        .rx
        .push_event(slider, EventSignal::SliderChanged { slider, value: 3.0 });
    update(&mut scene);
    assert_eq!(
        scene.systems.graphics_settings.foveation_quality(),
        XrFoveationQuality::High
    );
    assert_eq!(readout(&scene, a), "High");
    assert_eq!(readout(&scene, b), "High");

    let eye_control = named(&scene, Some(a), "graphics_foveation_eye_tracking_control");
    assert!(!scene.world.children_of(eye_control).iter().any(|&id| {
        scene
            .world
            .get_component_by_id_as::<ToggleComponent>(id)
            .is_some()
    }));
    assert!(scene.world.children_of(eye_control).iter().any(|&id| {
        scene
            .world
            .get_component_by_id_as::<RaycastableComponent>(id)
            .is_some_and(|ray| !ray.enable)
    }));
    click(&mut scene, eye_control);
    assert_eq!(
        scene.systems.graphics_settings.foveation_quality(),
        XrFoveationQuality::High
    );

    let toggle = named(&scene, Some(a), "accordion_toggle");
    click(&mut scene, toggle);
    assert!(scene.world.get_component_record(slider).is_none());
    scene
        .systems
        .graphics_settings
        .set_foveation_quality(XrFoveationQuality::Medium, &mut scene.command_queue);
    update(&mut scene);
    assert_eq!(readout(&scene, b), "Medium");
    click(&mut scene, toggle);
    assert_eq!(readout(&scene, a), "Medium");
    let slider = named(&scene, Some(a), "graphics_foveation_quality_slider");
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<SliderComponent>(slider)
            .unwrap()
            .value(),
        2.0
    );
    scene.systems.graphics_settings.reset();
    assert_eq!(
        scene.systems.graphics_settings.foveation_quality(),
        XrFoveationQuality::Off
    );
}

#[test]
fn foveation_builtin_round_trips_levels_and_rejects_invalid_names() {
    for name in ["off", "low", "medium", "high"] {
        let mut scene = Universe::new(World::default());
        scene
            .load_mms_source_at_path(
                &format!(
                    r#"
            GraphicsSettings.foveation.quality("{name}")
            Text {{ name="quality_readback" GraphicsSettings.foveation.quality_level() }}
        "#
                ),
                "examples/graphics-settings.mms",
            )
            .unwrap();
        let readback = named(&scene, None, "quality_readback");
        assert_eq!(
            scene
                .world
                .get_component_by_id_as::<TextComponent>(readback)
                .unwrap()
                .text,
            name
        );
    }
    for name in ["ultra", "LOW", "", "75"] {
        let mut scene = Universe::new(World::default());
        let error = scene
            .load_mms_source_at_path(
                &format!("GraphicsSettings.foveation.quality(\"{name}\")"),
                "examples/graphics-settings.mms",
            )
            .unwrap_err();
        assert!(
            error.contains("expects off, low, medium or high"),
            "{error}"
        );
        assert_eq!(
            scene.systems.graphics_settings.foveation_quality(),
            XrFoveationQuality::Off
        );
    }
}

#[test]
fn foveation_feedback_distinguishes_requested_accepted_failed_and_unavailable() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
        GraphicsSettings.foveation.quality("low")
        graphics_settings_panel({root_name="feedback_panel"})
    "#, "examples/graphics-settings.mms").unwrap();
    update(&mut scene);
    let panel = named(&scene, None, "feedback_panel");
    let status_text = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<TextComponent>(named(
                scene,
                Some(panel),
                "graphics_foveation_status",
            ))
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(status_text(&scene), "XR foveation backend unavailable.");
    scene.systems.graphics_settings.set_foveation_feedback(
        XrFoveationFeedback {
            status: XrFoveationStatus::Accepted,
            applied: Some(XrFoveationQuality::Low),
        },
        &mut scene.command_queue,
    );
    update(&mut scene);
    assert_eq!(status_text(&scene), "Applied: low");
    let slider = named(&scene, Some(panel), "graphics_foveation_quality_slider");
    scene
        .systems
        .rx
        .push_event(slider, EventSignal::SliderChanged { slider, value: 3.0 });
    update(&mut scene);
    assert_eq!(readout(&scene, panel), "High");
    assert_eq!(status_text(&scene), "Waiting to apply quality.");
    assert_eq!(
        scene.systems.graphics_settings.foveation_feedback().applied,
        Some(XrFoveationQuality::Low)
    );
    scene.systems.graphics_settings.set_foveation_feedback(
        XrFoveationFeedback {
            status: XrFoveationStatus::Failed,
            applied: Some(XrFoveationQuality::Low),
        },
        &mut scene.command_queue,
    );
    update(&mut scene);
    assert_eq!(status_text(&scene), "Request failed; last applied: low");
    scene
        .systems
        .graphics_settings
        .set_foveation_feedback(XrFoveationFeedback::default(), &mut scene.command_queue);
    update(&mut scene);
    assert_eq!(status_text(&scene), "XR foveation backend unavailable.");
    assert_eq!(
        readout(&scene, panel),
        "High",
        "unavailable backend must not erase preference"
    );
}
