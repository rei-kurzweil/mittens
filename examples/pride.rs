use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("pride.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors,
    );

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    spawn_pride_rainbow(&mut universe);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}

fn spawn_pride_rainbow(universe: &mut engine::Universe) {
    use engine::ecs::component::{
        ColorComponent, EmissiveComponent, RenderableComponent, TransformComponent,
    };
    use engine::graphics::mesh::MeshFactory;
    use engine::graphics::primitives::{MaterialHandle, Renderable};

    let center = [-2.1_f32, -2.1_f32, -4.0_f32];
    let band_thickness = 0.34_f32;
    let gap = 0.03_f32;
    let start_angle = 0.0_f32;
    let sweep = std::f32::consts::FRAC_PI_2;
    let segments = 48_u32;
    let colors = [
        [0.89, 0.16, 0.11, 1.0],
        [0.98, 0.49, 0.10, 1.0],
        [0.99, 0.84, 0.13, 1.0],
        [0.16, 0.68, 0.27, 1.0],
        [0.10, 0.42, 0.91, 1.0],
    ];

    for (layer, color) in colors.into_iter().enumerate() {
        let inner = 0.55 + layer as f32 * (band_thickness + gap);
        let outer = inner + band_thickness;
        let mesh = MeshFactory::partial_annulus_2d(inner, outer, start_angle, sweep, segments);
        let mesh_handle = universe.render_assets.register_mesh(mesh);

        let root = universe.world.add_component(
            TransformComponent::new().with_position(center[0], center[1], center[2]),
        );
        let renderable = universe
            .world
            .add_component(RenderableComponent::new(Renderable::new(
                mesh_handle,
                MaterialHandle::TOON_MESH,
            )));
        let color_component = universe
            .world
            .add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
        let emissive = universe.world.add_component(EmissiveComponent::on());

        let _ = universe.attach(root, renderable);
        let _ = universe.attach(renderable, color_component);
        let _ = universe.attach(renderable, emissive);
        universe.add(root);
    }
}
