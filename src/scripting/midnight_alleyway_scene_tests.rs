use crate::engine::Universe;
use crate::engine::ecs::component::material::{MaterialComponent, MaterialDefinitionComponent};
use crate::engine::ecs::component::{CameraXRComponent, GrabbableComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};

const PATH: &str = "examples/midnight-alleyway.mms";

fn named(scene: &Universe, name: &str) -> Option<ComponentId> {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
}

#[test]
fn midnight_alleyway_loads_xr_scene_on_the_top_storey() {
    let source = std::fs::read_to_string(PATH).unwrap();
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(&source, PATH).unwrap();
    for _ in 0..10 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let world = &scene.world;
    let count = |predicate: &dyn Fn(ComponentId) -> bool| {
        world.all_components().filter(|&id| predicate(id)).count()
    };
    assert_eq!(
        count(&|id| world.get_component_by_id_as::<CameraXRComponent>(id).is_some()),
        1
    );
    let panorama = count(&|id| {
        world
            .get_component_by_id_as::<MaterialComponent>(id)
            .and_then(|m| world.get_component_by_id_as::<MaterialDefinitionComponent>(m.definition))
            .is_some_and(|d| d.fragment_uri == "assets/shaders/panorama-cylinder.frag")
    });
    assert_eq!(panorama, 1);
    assert!(named(&scene, "panorama_cylinder_surface").is_some());
    assert!(named(&scene, "midnight_alleyway_building").is_some());
    for label in [
        "midnight_alleyway_tripod_warm",
        "midnight_alleyway_tripod_cool",
        "midnight_alleyway_tripod_pink",
    ] {
        let tripod = named(&scene, label).unwrap_or_else(|| panic!("missing {label}"));
        assert!(
            world.children_of(tripod).iter().any(|&child| world
                .get_component_by_id_as::<GrabbableComponent>(child)
                .is_some()),
            "{label} is not grabbable"
        );
    }
    // Spawn on the top storey of five: one floor (3.2 m) below the roof.
    let local = TransformSystem::world_position(world, named(&scene, "local_root").unwrap()).unwrap();
    assert!((local[1] - 12.8).abs() < 1e-3, "{local:?}");
}
