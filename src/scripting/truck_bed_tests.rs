//! Items released over the pickup bed attach to it and ride the truck.
use super::rocket_motion_tests::Uploader;
use crate::engine::Universe;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::transform::TransformSpace;

fn scene(source: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/_truck_bed_test.mms")
        .unwrap();
    step(&mut scene);
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut Uploader::default(),
        &mut scene.command_queue,
    );
    step(&mut scene);
    scene
}

fn step(scene: &mut Universe) {
    scene.update(0.0, &Default::default());
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn position(scene: &Universe, id: ComponentId) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, id).unwrap()
}

fn move_to(scene: &mut Universe, id: ComponentId, translation: [f32; 3]) {
    let mut value = TransformSystem::world_trs(&scene.world, id).unwrap();
    value.translation = translation;
    scene.command_queue.push_intent_now(
        id,
        IntentValue::SetTransformTrs {
            component_id: id,
            space: TransformSpace::World,
            value,
        },
    );
    step(scene);
}

fn grab(scene: &mut Universe, target: ComponentId) {
    let pointer = named(scene, "test_pointer");
    let renderable = scene.world.find_component(target, "renderable").unwrap();
    scene.command_queue.push_event(
        renderable,
        EventSignal::GrabStart {
            pointer,
            target,
            renderable,
            raycaster: pointer,
            ray_origin_world: [0.0, 0.0, 2.0],
            ray_dir_world: [0.0, 0.0, -1.0],
        },
    );
    step(scene);
    assert_eq!(scene.world.parent_of(target), Some(named(scene, "hand")));
}

fn release(scene: &mut Universe, target: ComponentId) {
    let pointer = named(scene, "test_pointer");
    scene
        .command_queue
        .push_event(target, EventSignal::GrabEnd { pointer, target });
    step(scene);
    step(scene);
}

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

#[test]
fn items_released_over_the_bed_stay_put_and_ride_the_truck() {
    let mut scene = scene(
        r#"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        T.position(10, 0, 0) { name = "truck_place" pickup_truck(null, null, false) }
        T { name = "hand" Pointer { name = "test_pointer" } }
        T.position(3, 1, 0) { name = "gun" Grabbable {} R.cube() }
        T.position(3, 1, 0) { name = "ammo" Grabbable {} R.cube() }
        T.position(3, 1, 0) { name = "spare" Grabbable {} R.cube() }
    "#,
    );
    let bed = named(&scene, "pickup_bed_cargo_zone");
    let center = position(&scene, named(&scene, "pickup_bed_cargo_frame"));
    let mut placed = Vec::new();
    // No item limit: every release over the bed attaches.
    for (name, offset) in [
        ("gun", [0.3, 0.0, 0.2]),
        ("ammo", [-0.3, 0.1, -0.4]),
        ("spare", [0.0, -0.2, 0.6]),
    ] {
        let item = named(&scene, name);
        let target = std::array::from_fn(|i| center[i] + offset[i]);
        grab(&mut scene, item);
        move_to(&mut scene, item, target);
        release(&mut scene, item);
        assert_eq!(scene.world.parent_of(item), Some(bed), "{name} attaches to the bed");
        assert!(near(position(&scene, item), target), "{name} stays where it was released");
        placed.push((item, target));
    }

    let place = named(&scene, "truck_place");
    move_to(&mut scene, place, [15.0, 0.0, -2.0]);
    for (item, target) in &placed {
        let expected = [target[0] + 5.0, target[1], target[2] - 2.0];
        assert!(near(position(&scene, *item), expected), "items ride the truck");
    }

    // Taking an item out and releasing it in the open leaves the bed.
    let (gun, _) = placed[0];
    grab(&mut scene, gun);
    move_to(&mut scene, gun, [3.0, 1.0, 0.0]);
    release(&mut scene, gun);
    assert_eq!(scene.world.parent_of(gun), None);
    assert!(near(position(&scene, gun), [3.0, 1.0, 0.0]));
}

fn zone_holds(scene: &Universe, zone: &str, point: [f32; 3]) -> bool {
    use crate::engine::ecs::system::{ZoneRelation, classify_zone_point};
    matches!(
        classify_zone_point(&scene.world, named(scene, zone), point),
        Ok(ZoneRelation::Inside | ZoneRelation::Boundary)
    )
}

#[test]
fn driver_passenger_and_bed_seats_share_the_truck_grip_by_entry_zone() {
    use crate::engine::ecs::component::MountableComponent;
    let scene = scene(
        r#"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        T { name = "truck_place" pickup_truck(null, null, false) }
    "#,
    );
    // All three grip seats live on the truck root, so a grip anywhere on the
    // truck reaches them and the rider's entry zone picks one.
    let truck = named(&scene, "pickup_truck");
    let seats = scene
        .world
        .children_of(truck)
        .iter()
        .filter(|&&child| {
            scene
                .world
                .get_component_by_id_as::<MountableComponent>(child)
                .is_some_and(|m| m.enabled && m.on_grip)
        })
        .count();
    assert_eq!(seats, 3, "driver, passenger and bed");
    let chassis = named(&scene, "pickup_chassis_frame");
    let model = TransformSystem::world_model(&scene.world, chassis).unwrap();
    let at = |p: [f32; 3]| {
        let w = crate::utils::math::mat4_mul_vec4(model, [p[0], p[1], p[2], 1.0]);
        [w[0], w[1], w[2]]
    };
    let zones = ["pickup_entry_zone", "pickup_passenger_entry_zone", "pickup_bed_entry_zone"];
    // Heads beside each door, standing in the bed, at a bed wall and
    // behind the tailgate, all in chassis space.
    for (point, expected) in [
        ([-1.2, 1.7, -0.25], "pickup_entry_zone"),
        ([1.2, 1.7, -0.25], "pickup_passenger_entry_zone"),
        ([0.0, 2.3, 1.7], "pickup_bed_entry_zone"),
        ([1.05, 1.7, 1.7], "pickup_bed_entry_zone"),
        ([-1.05, 1.7, 2.2], "pickup_bed_entry_zone"),
        ([0.0, 1.7, 2.85], "pickup_bed_entry_zone"),
    ] {
        let holders: Vec<&str> = zones
            .iter()
            .copied()
            .filter(|zone| zone_holds(&scene, zone, at(point)))
            .collect();
        assert_eq!(holders, vec![expected], "head at {point:?}");
    }
    // The cab interior and the hood belong to no seat.
    for point in [[0.0, 1.7, -0.25], [0.0, 1.5, -1.6]] {
        assert!(
            zones.iter().all(|zone| !zone_holds(&scene, zone, at(point))),
            "{point:?}"
        );
    }
}

#[test]
fn cab_doors_hang_on_grabbable_hinges_and_keep_their_colliders() {
    use crate::engine::ecs::component::{
        CollidableComponent, GrabbableComponent, TransformOperatorComponent,
    };
    let scene = scene(
        r#"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        T { name = "truck_place" pickup_truck(null, null, false) }
    "#,
    );
    let world = &scene.world;
    for name in ["door_left", "door_right"] {
        let door = named(&scene, name);
        let hinge = world.parent_of(door).expect("door has a parent");
        assert!(
            world
                .get_component_by_id_as::<TransformOperatorComponent>(hinge)
                .and_then(|operator| operator.hinge_settings())
                .is_some(),
            "{name} hangs directly under a hinge"
        );
        assert!(
            world
                .children_of(door)
                .iter()
                .any(|&child| world.get_component_by_id_as::<GrabbableComponent>(child).is_some()),
            "{name} is grabbable"
        );
        let panel = world.find_component(door, "renderable").unwrap();
        assert!(
            world
                .children_of(panel)
                .iter()
                .any(|&child| world.get_component_by_id_as::<CollidableComponent>(child).is_some()),
            "{name} keeps its panel collider"
        );
    }
}
