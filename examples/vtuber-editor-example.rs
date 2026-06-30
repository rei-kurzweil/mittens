use cat_engine::engine::ecs::component::{ControllerHand, ControllerXRComponent};
use cat_engine::engine::ecs::{ComponentId, EventSignal, Signal, SignalEmitter, SignalKind, World};
use cat_engine::{engine, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn component_label(world: &World, id: ComponentId) -> String {
    world.component_name(id).unwrap_or("?").to_string()
}

fn controller_hand_for_component(world: &World, start: ComponentId) -> Option<ControllerHand> {
    let mut cur = start;
    loop {
        if let Some(ctrl) = world.get_component_by_id_as::<ControllerXRComponent>(cur) {
            return Some(ctrl.hand);
        }
        cur = world.parent_of(cur)?;
    }
}

fn on_xr_pointer_event(world: &mut World, _emit: &mut dyn SignalEmitter, env: &Signal) {
    let Some(event) = env.event.as_ref() else {
        return;
    };
    let (kind, raycaster, renderable, hit_point) = match event {
        EventSignal::DragStart {
            raycaster,
            renderable,
            hit_point,
            ..
        } => ("DragStart", *raycaster, *renderable, Some(*hit_point)),
        EventSignal::DragMove {
            raycaster,
            renderable,
            hit_point,
            ..
        } => ("DragMove", *raycaster, *renderable, Some(*hit_point)),
        EventSignal::DragEnd {
            raycaster,
            renderable,
            hit_point,
        } => ("DragEnd", *raycaster, *renderable, *hit_point),
        EventSignal::Click {
            raycaster,
            renderable,
            hit_point,
            ..
        } => ("Click", *raycaster, *renderable, Some(*hit_point)),
        _ => return,
    };

    let Some(hand) = controller_hand_for_component(world, raycaster) else {
        return;
    };
    let hand_label = match hand {
        ControllerHand::Left => "Left",
        ControllerHand::Right => "Right",
    };
    let renderable_name = component_label(world, renderable);
    let ray_name = component_label(world, raycaster);
    if let Some(p) = hit_point {
        println!(
            "[xr-pointer] hand={} kind={} raycaster={} renderable={} hit=[{:+.3},{:+.3},{:+.3}]",
            hand_label, kind, ray_name, renderable_name, p[0], p[1], p[2]
        );
    } else {
        println!(
            "[xr-pointer] hand={} kind={} raycaster={} renderable={}",
            hand_label, kind, ray_name, renderable_name
        );
    }
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("vtuber-editor-example.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from vtuber-editor-example.mms",
        output.intents.len()
    );
    println!(
        "[vtuber-editor-example] expected XR-only views: 2 XR eye views plus mirror-derived views; no desktop scene view unless a Camera3D/Camera2D is added"
    );

    let world = engine::ecs::World::default();

    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);

    let cloud_params = example_util::CloudRingParams {
        cloud_count: 10,
        radius: 34.0,
        center_y: 8.5,
        puffs_per_cloud: 28,
        angle_jitter: 0.30,
        high_y_probability: 0.45,
        high_y_multiplier: 1.28,
        seed: 0x57_55_B0_0Au32,
    };
    example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params);

    for kind in [
        SignalKind::DragStart,
        SignalKind::DragMove,
        SignalKind::DragEnd,
        SignalKind::Click,
    ] {
        universe
            .systems
            .rx
            .add_global_handler(kind, on_xr_pointer_event);
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
