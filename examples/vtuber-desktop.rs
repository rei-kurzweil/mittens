use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn attach_desktop_pointer_to_first_camera(universe: &mut engine::Universe) {
    use engine::ecs::component::{Camera3DComponent, PointerComponent, RayCastComponent};

    let camera = universe.world.all_components().find(|&cid| {
        universe
            .world
            .get_component_by_id_as::<Camera3DComponent>(cid)
            .is_some()
    });

    let Some(camera) = camera else {
        eprintln!("[vtuber-desktop] warning: no Camera3DComponent found for editor pointer");
        return;
    };

    let Some(camera_rig) = universe.world.parent_of(camera) else {
        eprintln!(
            "[vtuber-desktop] warning: Camera3DComponent has no parent transform for editor pointer"
        );
        return;
    };

    let existing_raycast = universe.world.children_of(camera_rig).iter().copied().find(|&cid| {
        universe
            .world
            .get_component_by_id_as::<RayCastComponent>(cid)
            .is_some()
    });

    let raycast = existing_raycast.unwrap_or_else(|| {
        let raycast = universe
            .world
            .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
        let _ = universe.attach(camera_rig, raycast);
        raycast
    });

    let has_pointer = universe.world.children_of(raycast).iter().any(|&cid| {
        universe
            .world
            .get_component_by_id_as::<PointerComponent>(cid)
            .is_some()
    });

    if !has_pointer {
        let pointer = universe.world.add_component(PointerComponent::new());
        let _ = universe.attach(raycast, pointer);
    }
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("vtuber-desktop.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!("[mms] {} intent(s) from vtuber-desktop.mms", output.intents.len());

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    attach_desktop_pointer_to_first_camera(&mut universe);

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
