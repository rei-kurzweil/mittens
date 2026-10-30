use mittens_engine::engine::{
    Universe,
    ecs::{World, component::RendererSettingsComponent},
};

fn resolution(source: &str) -> Result<Option<[u32; 2]>, String> {
    let mut universe = Universe::new(World::default());
    universe.load_mms_source_at_path(source, "android/examples/sakura-stage.mms")?;
    Ok(universe
        .world
        .all_components()
        .filter_map(|id| {
            universe
                .world
                .get_component_by_id_as::<RendererSettingsComponent>(id)
        })
        .last()
        .and_then(|settings| settings.desired_xr_resolution))
}

#[test]
fn mms_xr_resolution_is_read_from_the_scene() {
    assert_eq!(
        resolution("RendererSettings { desired_xr_resolution([1712, 1800]) }").unwrap(),
        Some([1712, 1800])
    );
    assert_eq!(
        resolution("RendererSettings.desired_xr_resolution([1000, 900]) {}").unwrap(),
        Some([1000, 900])
    );
    assert_eq!(resolution("RendererSettings {}").unwrap(), None);
    for value in ["[0, 2]", "[-1, 2]", "[1.5, 2]", "[1]", "[1, 2, 3]"] {
        assert!(
            resolution(&format!(
                "RendererSettings {{ desired_xr_resolution({value}) }}"
            ))
            .is_err(),
            "accepted {value}"
        );
    }
}
