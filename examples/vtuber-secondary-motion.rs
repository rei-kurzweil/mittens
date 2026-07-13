use cat_engine::{
    engine,
    engine::ecs::SignalEmitter,
    engine::ecs::component::{
        GLTFComponent, SecondaryMotionComponent, SpringBoneComponent, SpringJointComponent,
    },
    meow_meow, utils,
};

fn main() {
    utils::logger::init();
    let output = meow_meow::MeowMeowRunner::eval(include_str!("vtuber-secondary-motion.mms"));
    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
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
    for strand in 1..=14 {
        let chain = universe.world.add_component(
            SpringBoneComponent::new(format!("hair_{strand:02}")).virtual_end_length_ratio(1.0),
        );
        universe.world.add_child(metadata, chain).unwrap();
        for segment in 1..=3 {
            let path = format!(
                "Armature.003[0]/Root[0]/J_Bip_C_Hips[0]/J_Bip_C_Spine[0]/J_Bip_C_Chest[0]/J_Bip_C_UpperChest[0]/J_Bip_C_Neck[0]/J_Bip_C_Head[0]/J_Sec_Hair{segment}_{strand:02}[0]"
            );
            let joint = universe.world.add_component(
                SpringJointComponent::new(path)
                    .stiffness(1.0)
                    .drag_force(0.4)
                    .gravity(0.0, [0.0, -1.0, 0.0]),
            );
            universe.world.add_child(chain, joint).unwrap();
        }
    }
    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
