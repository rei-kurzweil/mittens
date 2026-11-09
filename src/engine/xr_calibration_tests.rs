use super::*;
use crate::engine::ecs::system::xr_settings_system::calibration_file;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mittens-xr-session-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn file(&self) -> String {
        self.0
            .join("floor calibration.json")
            .to_str()
            .unwrap()
            .into()
    }
    fn quoted(&self) -> String {
        serde_json::to_string(&self.file()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn xr_calibration_file_api_read_is_explicit_and_failed_save_preserves_runtime() {
    let fixture = Fixture::new();
    let file = fixture.quoted();
    calibration_file::save(&fixture.file(), -0.15).unwrap();
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(
            &format!(
                r#"
        XrSettings.floor.set_correction_meters(0.2)
        Text {{ name = "saved_floor" "" + XrSettings.floor.correction_meters({{file = {file}}}) }}
    "#
            ),
            "examples/_xr_file_test.mms",
        )
        .unwrap();
    assert!((universe.systems.xr_settings.floor_correction_meters() - 0.2).abs() < 1e-6);
    let text = universe
        .world
        .get_component_by_id_as::<ecs::component::TextComponent>(labelled(&universe, "saved_floor"))
        .unwrap();
    assert_eq!(text.text.parse::<f64>().unwrap(), -0.15);
    let blocked = serde_json::to_string(&format!("{}/child.json", fixture.file())).unwrap();
    for source in [
        format!("XrSettings.floor.set_correction_meters(0.3, {{file = {blocked}}})"),
        format!("XrSettings.floor.set_correction_meters(0.51, {{file = {file}}})"),
        "XrSettings.floor.correction_meters({})".into(),
        "XrSettings.floor.correction_meters({file = 42})".into(),
        format!("XrSettings.floor.correction_meters({{file = {file}, typo = true}})"),
    ] {
        assert!(
            universe
                .load_mms_source_at_path(&source, "examples/_xr_file_test.mms")
                .is_err(),
            "{source}"
        );
        assert!((universe.systems.xr_settings.floor_correction_meters() - 0.2).abs() < 1e-6);
        assert_eq!(calibration_file::load(&fixture.file()).unwrap(), -0.15);
    }
}

fn panel_scene(file: &str, root: &str) -> String {
    format!(
        r#"
        import {{ xr_settings_panel }} from "../assets/components/panels/xr_settings_panel.mms"
        let calibration_file = {file}
        XrSettings.floor.set_correction_meters(XrSettings.floor.correction_meters({{file = calibration_file}}))
        xr_settings_panel({{root_name = "{root}", calibration_file = calibration_file}})
    "#
    )
}

#[test]
fn xr_calibration_panel_commits_share_across_scenes_restarts_and_reset() {
    let fixture = Fixture::new();
    let first_scene = panel_scene(&fixture.quoted(), "first_demo");
    let second_scene = panel_scene(&fixture.quoted(), "second_demo");
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(&first_scene, "examples/_xr_first.mms")
        .unwrap();
    assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
    let slider = labelled(&universe, "xr_floor_correction_slider");
    universe.command_queue.push_intent_now(
        slider,
        ecs::IntentValue::SliderSet {
            component_id: slider,
            value: 0.125,
            emit_changed: true,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert!((universe.systems.xr_settings.floor_correction_meters() - 0.125).abs() < 1e-6);
    assert!(
        !std::path::Path::new(&fixture.file()).exists(),
        "drag preview must not write to disk"
    );
    universe.command_queue.push_event(
        slider,
        EventSignal::SliderCommitted {
            slider,
            value: 0.125,
        },
    );
    pump_settings_callbacks(&mut universe);
    assert_eq!(calibration_file::load(&fixture.file()).unwrap(), 0.125);
    universe
        .load_mms_source_at_path(&second_scene, "examples/_xr_second.mms")
        .unwrap();
    labelled(&universe, "second_demo");
    assert_eq!(
        universe.systems.xr_settings.floor_correction_meters(),
        0.125
    );
    // A fresh Universe simulates restarting the application.
    drop(universe);
    let mut universe = Universe::new(ecs::World::default());
    universe
        .load_mms_source_at_path(&first_scene, "examples/_xr_first.mms")
        .unwrap();
    assert_eq!(
        universe.systems.xr_settings.floor_correction_meters(),
        0.125
    );
    // Rebuilt accordion callbacks retain the selected calibration file.
    let toggle = labelled(&universe, "accordion_toggle");
    click(&mut universe, toggle);
    click(&mut universe, toggle);
    let reset = labelled(&universe, "xr_floor_correction_reset");
    click(&mut universe, reset);
    assert_eq!(calibration_file::load(&fixture.file()).unwrap(), 0.0);
    universe
        .load_mms_source_at_path(&second_scene, "examples/_xr_second.mms")
        .unwrap();
    assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
}

#[test]
fn xr_calibration_two_real_demos_load_the_shared_saved_default() {
    let fixture = Fixture::new();
    calibration_file::save(&fixture.file(), -0.075).unwrap();
    let mut universe = Universe::new(ecs::World::default());
    for (source, path) in [
        (
            include_str!("../../examples/xr-settings.mms"),
            "examples/xr-settings.mms",
        ),
        (
            include_str!("../../examples/lower-body-walking-vr.mms"),
            "examples/lower-body-walking-vr.mms",
        ),
    ] {
        // Exercise authored startup without reading or modifying personal config.
        let source = source.replace("XrSettings.floor.default_file()", &fixture.quoted());
        universe.load_mms_source_at_path(&source, path).unwrap();
        assert!((universe.systems.xr_settings.floor_correction_meters() + 0.075).abs() < 1e-6);
        let slider = universe
            .world
            .get_component_by_id_as::<ecs::component::SliderComponent>(labelled(
                &universe,
                "xr_floor_correction_slider",
            ))
            .unwrap();
        assert!((slider.value() + 0.075).abs() < 1e-6);
    }
}
