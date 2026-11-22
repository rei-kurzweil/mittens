use super::*;
use crate::engine::ecs::component::{GLTFComponent, SliderComponent, TextComponent};

#[test]
fn avatar_settings_panel_commits_height_and_preserves_xr_calibration() {
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            r##"
            import { avatar_settings_panel } from "../assets/components/panels/avatar_settings_panel.mms"
            avatar_settings_panel({ root_name = "avatar_settings_test" })
            T { name = "test_models" }
            on_global("DataEvent", fn(event) {
                if event == "AvatarSettingsChanged" {
                    let model = GLTF.new("assets/models/bisket.glb").target_height(AvatarSettings.height.meters()) {
                        name = "resized_model"
                    }
                    query("#test_models").attach(model)
                }
            })
            "##,
            "examples/_avatar_settings_test.mms",
        )
        .unwrap();
    let height = labelled(&universe, "avatar_height_slider");
    let floor = labelled(&universe, "xr_floor_correction_slider");
    for slider in [height, floor] {
        let slider = universe
            .world
            .get_component_by_id_as::<SliderComponent>(slider)
            .unwrap();
        assert!(slider.width() >= 2.0 * 0.65);
    }
    universe.command_queue.push_intent_now(
        floor,
        ecs::IntentValue::SliderSet {
            component_id: floor,
            value: 0.15,
            emit_changed: true,
        },
    );
    universe.command_queue.push_intent_now(
        height,
        ecs::IntentValue::SliderSet {
            component_id: height,
            value: 1.8,
            emit_changed: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert_eq!(
        universe.systems.avatar_settings.desired_height_meters(),
        None
    );
    assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
    universe.command_queue.push_event(
        height,
        EventSignal::SliderCommitted {
            slider: height,
            value: 1.8,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert_eq!(
        universe.systems.avatar_settings.desired_height_meters(),
        Some(1.8)
    );
    assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
    let readout = labelled(&universe, "avatar_height_readout");
    assert_eq!(
        universe
            .world
            .get_component_by_id_as::<TextComponent>(readout)
            .unwrap()
            .text,
        "1.80 m"
    );
    let model = labelled(&universe, "resized_model");
    universe.update(1.0 / 60.0, &Default::default());
    let model = universe
        .world
        .get_component_by_id_as::<GLTFComponent>(model)
        .unwrap();
    assert!(model.spawned);
    assert_eq!(model.default_height_if_outside, Some([1.8; 3]));
    assert!(model.import_scale > 1.0);
}
