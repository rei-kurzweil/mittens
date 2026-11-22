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

/// Mirrors the attachment system's grip resolution: the first transform up
/// the hit's ancestry with an enabled Mountable child.
fn mountable_above(scene: &Universe, hit: ComponentId) -> Option<ComponentId> {
    use crate::engine::ecs::component::{MountableComponent, TransformComponent};
    let world = &scene.world;
    let mut current = Some(hit);
    while let Some(id) = current {
        if world.get_component_by_id_as::<TransformComponent>(id).is_some() {
            if let Some(&mountable) = world.children_of(id).iter().find(|&&child| {
                world
                    .get_component_by_id_as::<MountableComponent>(child)
                    .is_some_and(|m| m.enabled && m.on_grip)
            }) {
                return Some(mountable);
            }
        }
        current = world.parent_of(id);
    }
    None
}

#[test]
fn bench_grip_resolves_the_bed_seat_and_the_cab_keeps_the_driver_seat() {
    let scene = scene(
        r#"
        import { pickup_truck } from "../assets/components/vehicles/pickup_truck.mms"
        T { name = "truck_place" pickup_truck(null, null, false) }
    "#,
    );
    let bed_seat = named(&scene, "pickup_bed_mountable");
    let bench = named(&scene, "pickup_bed_bench");
    let board = scene.world.find_component(bench, "renderable").unwrap();
    assert_eq!(mountable_above(&scene, board), Some(bed_seat));

    let seat = named(&scene, "car_seat");
    let cab = scene.world.find_component(seat, "renderable").unwrap();
    let driver = mountable_above(&scene, cab).expect("the cab resolves a seat");
    assert_ne!(driver, bed_seat, "the cab must keep resolving to the driver's seat");
}
