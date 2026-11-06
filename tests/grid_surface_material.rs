use mittens_engine::engine::{
    Universe,
    ecs::{
        World,
        component::material::{
            MaterialBaseComponent, MaterialComponent, MaterialDefinitionComponent,
        },
    },
    graphics::{custom_material, materials::MaterialBaseKind},
};

#[test]
fn grid_surface_uses_shared_standard_definition_and_named_spacing_inputs() {
    let mut scene = Universe::new(World::default());
    let source = std::fs::read_to_string("examples/grid-material.mms").unwrap();
    scene
        .load_mms_source_at_path(&source, "examples/grid-material.mms")
        .unwrap();
    for _ in 0..4 {
        scene.update(0., &Default::default());
    }
    let components: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| scene.world.get_component_by_id_as::<MaterialComponent>(id))
        .collect();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].definition, components[1].definition);
    let definition = scene
        .world
        .get_component_by_id_as::<MaterialDefinitionComponent>(components[0].definition)
        .unwrap();
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<MaterialBaseComponent>(definition.base)
            .unwrap()
            .0,
        MaterialBaseKind::Surface
    );
    assert!(
        components
            .iter()
            .any(|m| m.values["spacing"] == vec![0.25] && m.values["world_space"] == vec![0.])
    );
    assert!(
        components
            .iter()
            .any(|m| m.values["spacing"] == vec![0.35] && m.values["world_space"] == vec![1.])
    );
    assert!(!definition.fields.iter().any(|f| f.name.contains("quant")));
    let compiled = custom_material::compile(&definition.fragment_uri, &definition.fields).unwrap();
    assert!(compiled.byte_size > 0);
    assert_eq!(
        compiled.offsets.len(),
        definition
            .fields
            .iter()
            .map(|field| field.ty.width())
            .sum::<usize>()
    );
}
