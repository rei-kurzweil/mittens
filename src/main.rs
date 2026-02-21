use cat_engine::{engine, example, utils};

fn main() {
    utils::logger::init();

    // Parse CLI arguments
    let cli = engine::CLI::parse();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Handle load command before building scene
    if let engine::cli::CliCommand::Load { ref filename } = cli.command {
        println!("[CLI] Loading scene from '{}'...", filename);
        match engine::ecs::ComponentCodec::decode_scene(&mut universe.world, filename) {
            Ok(root_ids) => {
                println!(
                    "[CLI] Scene loaded successfully. {} root(s) loaded.",
                    root_ids.len()
                );
                // Initialize all loaded component trees
                for root_id in root_ids {
                    universe
                        .world
                        .init_component_tree(root_id, &mut universe.command_queue);
                }
                // Process any init commands
                universe.systems.process_commands(
                    &mut universe.world,
                    &mut universe.visuals,
                    &mut universe.command_queue,
                );
            }
            Err(e) => {
                eprintln!("[CLI] Failed to load scene: {}", e);
                eprintln!("[CLI] Building demo scene instead...");
                example::build_demo_scene_7_shapes(&mut universe);
            }
        }
    } else {
        // Build demo scene if not loading
        example::build_demo_scene_7_shapes(&mut universe);
    }

    // Handle save command after scene is built
    if let engine::cli::CliCommand::Save { ref filename } = cli.command {
        println!("[CLI] Saving scene to '{}'...", filename);

        // Find all root components (components with no parent)
        let root_components: Vec<engine::ecs::ComponentId> = universe
            .world
            .all_components()
            .filter(|&cid| universe.world.parent_of(cid).is_none())
            .collect();

        if root_components.is_empty() {
            eprintln!("[CLI] No root components found to save.");
        } else {
            println!("[CLI] Found {} root component(s)", root_components.len());

            match engine::ecs::ComponentCodec::encode_scene(
                &universe.world,
                &root_components,
                filename,
            ) {
                Ok(()) => println!(
                    "[CLI] Saved {} roots to '{}'",
                    root_components.len(),
                    filename
                ),
                Err(e) => eprintln!("[CLI] Failed to save scene: {}", e),
            }
        }

        // Exit after saving (don't run the window)
        println!("[CLI] Save complete. Exiting.");
        return;
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
