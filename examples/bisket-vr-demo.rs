use cat_engine::engine::ecs::SignalEmitter;
use cat_engine::engine::ecs::component::{
    ColorComponent, EmissiveComponent, OverlayComponent, RaycastableComponent,
    RenderableComponent, TransformComponent,
};
use cat_engine::{engine, meow_meow, utils};

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("bisket-vr-demo.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!("[mms] {} intent(s) from bisket-vr-demo.mms", output.intents.len());

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

    // Force the glTF subtree to spawn so the armature exists before we query for
    // bone markers (otherwise the bone components aren't in the world yet).
    {
        let systems = &mut universe.systems;
        systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut systems.skinned_mesh,
            &mut universe.command_queue,
            0.0,
        );
    }
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Bone markers — small cubes attached as children of named bones, so they
    // visibly track the bone's world pose. Useful for debugging where each
    // bone actually sits relative to the XR camera and the overlay cube.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']",      (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']",      (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']",(0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Sec_Hair1_08']",    (1.00, 0.00, 0.00, 1.0)),
        ("[name='J_Sec_Hair2_08']",    (1.00, 0.40, 0.40, 1.0)),
    ];

    // Find any global root to start the descendant search from. Scanning all
    // components is fine here — this only runs once at startup.
    let roots: Vec<engine::ecs::ComponentId> = universe
        .world
        .all_components()
        .filter(|&id| universe.world.parent_of(id).is_none())
        .collect();

    for &(selector, color) in marker_joints {
        let bone = roots
            .iter()
            .find_map(|&r| universe.find_component(r, selector));
        let Some(bone) = bone else {
            eprintln!("[bisket-vr-demo] bone not found: {selector}");
            continue;
        };
        // Topology:  bone -> OV -> marker_t -> marker_r (+C +EM +Raycastable)
        // OV is a phase marker: its subtree renders in the overlay pass (drawn
        // last, on top of everything). Without this the marker cube is occluded
        // by the avatar's head/body mesh in first-person VR.
        let marker_ov = universe.world.add_component(OverlayComponent::new());
        let marker_t = universe
            .world
            .add_component(TransformComponent::new().with_scale(0.025, 0.025, 0.025));
        let marker_r = universe.world.add_component(RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));
        let marker_rcast = universe.world.add_component(RaycastableComponent::enabled());
        let marker_em = universe.world.add_component(EmissiveComponent::on());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_em);
        let _ = universe.world.add_child(marker_r, marker_rcast);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.world.add_child(marker_ov, marker_t);
        let _ = universe.attach(bone, marker_ov);
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
