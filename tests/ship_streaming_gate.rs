use mittens_engine::engine::{
    Universe,
    ecs::{ComponentId, EventSignal, SignalEmitter, SignalKind, World, component::*},
};
fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn count(world: &World, label: &str) -> usize {
    world
        .all_components()
        .filter(|&id| world.component_label(id) == Some(label))
        .count()
}
fn tick(scene: &mut Universe) {
    for _ in 0..6 {
        scene.update(0.02, &Default::default());
    }
}
fn command(scene: &mut Universe, name: &str) {
    let player = named(&scene.world, "ship_desktop_camera_rig");
    scene.command_queue.push_event(
        player,
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
    tick(scene);
}
fn finish_materials(scene: &Universe) {
    let definitions: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            let registry = &scene.visuals.materials;
            registry
                .definition(registry.source_definition(id)?)?
                .custom
                .clone()
        })
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while definitions
        .iter()
        .any(|definition| definition.compiled.get().is_none())
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for definition in definitions {
        assert!(definition.compiled.get().unwrap().is_ok());
    }
}
fn descendants(world: &World, root: ComponentId) -> Vec<ComponentId> {
    let mut result = vec![];
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        result.push(id);
        pending.extend_from_slice(world.children_of(id));
    }
    result
}
fn lens_intensity(scene: &Universe, section: &str, color: &str) -> f32 {
    let root = named(&scene.world, section);
    let lens = scene
        .world
        .find_component(root, &format!("#traffic_light_{color}"))
        .unwrap();
    descendants(&scene.world, lens)
        .into_iter()
        .find_map(|id| {
            scene
                .world
                .get_component_by_id_as::<EmissiveComponent>(id)
                .map(|emission| emission.intensity)
        })
        .unwrap()
}
#[test]
fn doorway_streams_real_vehicle_groups_and_barracks_unload_their_subscriptions() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene.load_mms_source_at_path(r##"
        import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
        ship_input_controller(false)
        import { ship_store_accordion } from "../assets/components/ship_store_accordion.mms"
        import { ship_park_streaming_gate } from "../assets/components/rooms/ship_park_streaming_gate.mms"
        import { ship_motorhome_group,ship_driving_vehicle_group } from "../assets/components/vehicles/ship_vehicle_groups.mms"
        import { ship_barracks } from "../assets/components/rooms/ship_barracks.mms"
        fn shop_a() {return T {name="streamed_shop_a"}}
        fn shop_b() {return T {name="streamed_shop_b"}}
        T.position(100,5.2,0) {
            ship_store_accordion("ship_convenience_shop","CONVENIENCE",shop_a,null,null,[0,2,0],false)
            ship_store_accordion("ship_liquor_shop","LIQUOR",shop_b,null,null,[3,2,0],false)
            ship_store_accordion("ship_motorhomes","MOTORHOMES",ship_motorhome_group,null,null,[6,2,0],false)
            ship_store_accordion("ship_driving_vehicles","VEHICLES",ship_driving_vehicle_group,null,null,[9,2,0],false)
        }
        T.position(-20,0,0) {ship_barracks()}
        ship_park_streaming_gate()
        let player=T.position(8,6.45,15) {
            name="ship_desktop_camera_rig"
            Zone.sphere(0.2) {name="test_stream_player_zone" Collidable.slide().movement_target("/#ship_desktop_camera_rig") {}}
        }
        player
        on(player,"DataEvent",fn(event) {
            if event=="Inside" {player.update_transform([9.8,6.45,15])}
            if event=="Outside" {player.update_transform([12.2,6.45,15])}
            if event=="UnloadBarracks" {query("#ship_barracks_inventory").set_expanded(false)}
            if event=="LoadBarracks" {query("#ship_barracks_inventory").set_expanded(true)}
        })
    "##,"examples/_ship_streaming_test.mms").unwrap();
    finish_materials(&scene);
    tick(&mut scene);
    assert_eq!(count(&scene.world, "recreational_vehicle"), 0);
    assert_eq!(count(&scene.world, "pickup_truck"), 0);
    assert_eq!(count(&scene.world, "streamed_shop_a"), 0);
    assert!(scene.systems.rx.has_global_handlers(SignalKind::FrameTick));
    assert!(lens_intensity(&scene, "ship_motorhomes", "red") > 1.0);
    for _ in 0..3 {
        command(&mut scene, "Inside");
        assert_eq!(count(&scene.world, "pickup_truck"), 0);
        command(&mut scene, "Outside");
        assert_eq!(count(&scene.world, "recreational_vehicle"), 4);
        assert_eq!(count(&scene.world, "pickup_truck"), 3);
        assert_eq!(count(&scene.world, "motorcycle"), 2);
        assert_eq!(count(&scene.world, "streamed_shop_a"), 1);
        assert_eq!(count(&scene.world, "streamed_shop_b"), 1);
        assert!(lens_intensity(&scene, "ship_motorhomes", "green") > 1.0);
        let vehicles: Vec<_> = scene
            .world
            .all_components()
            .filter(|&id| {
                matches!(
                    scene.world.component_label(id),
                    Some("pickup_truck" | "motorcycle" | "recreational_vehicle")
                )
            })
            .collect();
        for vehicle in vehicles {
            assert!(
                scene
                    .world
                    .get_component_by_id_as::<VelocityComponent>(
                        scene.world.parent_of(vehicle).unwrap()
                    )
                    .is_some()
            );
        }
        command(&mut scene, "Outside"); // staying within the same sensor must not duplicate anything
        assert_eq!(count(&scene.world, "recreational_vehicle"), 4);
        command(&mut scene, "Inside");
        assert_eq!(count(&scene.world, "motorcycle"), 0);
        assert_eq!(count(&scene.world, "pickup_truck"), 0);
        assert_eq!(count(&scene.world, "streamed_shop_a"), 0);
        assert!(
            scene.systems.rx.has_global_handlers(SignalKind::FrameTick),
            "the single persistent controller remains while vehicle contents unload"
        );
        assert!(lens_intensity(&scene, "ship_motorhomes", "red") > 1.0);
    }
    assert_eq!(count(&scene.world, "ship_grocery_bananas"), 1);
    assert_eq!(count(&scene.world, "ship_grocery_oranges"), 1);
    assert_eq!(count(&scene.world, "ship_grocery_checkout"), 1);
    assert_eq!(count(&scene.world, "retail_shelf"), 5);
    let flights = |world: &World| {
        world
            .all_components()
            .filter(|&id| {
                world.component_label(id).is_some_and(|name| {
                    name.starts_with("ship_rack_pistol_") && name.ends_with("_flight")
                })
            })
            .count()
    };
    assert_eq!(flights(&scene.world), 9);
    command(&mut scene, "UnloadBarracks");
    assert_eq!(count(&scene.world, "retail_shelf"), 0);
    assert_eq!(
        flights(&scene.world),
        0,
        "projectile slots belong to the removable barracks assembly"
    );
    command(&mut scene, "LoadBarracks");
    assert_eq!(count(&scene.world, "retail_shelf"), 5);
    assert_eq!(flights(&scene.world), 9);
    finish_materials(&scene);
}
