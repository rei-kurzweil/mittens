use cat_engine::{
    engine,
    engine::ecs::SignalEmitter,
    engine::ecs::component::{
        GLTFComponent, SecondaryMotionComponent, SpringBoneComponent, SpringJointComponent,
    },
    engine::ecs::{ComponentId, World},
    meow_meow, utils,
};

fn attach_chain(
    world: &mut World,
    metadata: ComponentId,
    name: String,
    joints: impl IntoIterator<Item = String>,
    stiffness: f32,
    drag_force: f32,
    gravity_power: f32,
) {
    let chain = world.add_component(SpringBoneComponent::new(name).virtual_end_length_ratio(1.0));
    world.add_child(metadata, chain).unwrap();
    for node_name in joints {
        let joint = world.add_component(
            SpringJointComponent::query(format!("[name='{node_name}']"))
                .stiffness(stiffness)
                .drag_force(drag_force)
                .gravity(gravity_power, [0.0, -1.0, 0.0]),
        );
        world.add_child(chain, joint).unwrap();
    }
}

fn main() {
    utils::logger::init();
    let output = meow_meow::MeowMeowRunner::eval(include_str!("vtuber-secondary-motion.mms"));
    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors
    );
    let mut universe = engine::Universe::new(engine::ecs::World::default());
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
    let gltf = universe
        .world
        .all_components()
        .find(|id| {
            universe
                .world
                .get_component_by_id_as::<GLTFComponent>(*id)
                .map(|g| g.uri.contains("bisket"))
                .unwrap_or(false)
        })
        .expect("bisket GLTF missing");
    let metadata = universe
        .world
        .add_component(SecondaryMotionComponent::new());
    universe.world.add_child(gltf, metadata).unwrap();
    // Gravity is integrated with dt² while stiffness is integrated with dt, so
    // gravity_power must be numerically much larger than stiffness to produce a
    // visibly gravity-dominated equilibrium at 60 Hz.
    const FOUR_JOINT_STRANDS: &[usize] = &[1, 4, 5, 6, 7, 13];
    for strand in 1..=14 {
        let segment_count = if FOUR_JOINT_STRANDS.contains(&strand) {
            4
        } else {
            3
        };
        attach_chain(
            &mut universe.world,
            metadata,
            format!("hair_{strand:02}"),
            (1..=segment_count).map(|segment| format!("J_Sec_Hair{segment}_{strand:02}")),
            1.0,
            0.35,
            3.0,
        );
    }

    // The model calls these bust joints; they are the secondary chest chains.
    for side in ["L", "R"] {
        attach_chain(
            &mut universe.world,
            metadata,
            format!("{}_bust", side.to_ascii_lowercase()),
            [format!("J_Sec_{side}_Bust1"), format!("J_Sec_{side}_Bust2")],
            2.0,
            0.60,
            1.0,
        );
    }
    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
