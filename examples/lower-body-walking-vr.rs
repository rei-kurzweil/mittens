use mittens_engine::engine::ecs::component::{AvatarControlComponent, CameraXRComponent};
use mittens_engine::{engine, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let validate = args.iter().any(|a| a == "--validate");
    let model = args
        .windows(2)
        .find(|a| a[0] == "--model")
        .map(|a| a[1].as_str())
        .unwrap_or("bisket");
    let asset = match model {
        "bisket" => "assets/models/bisket.glb",
        "rei" => "assets/models/rei(mu).glb",
        _ => panic!("--model expects bisket or rei"),
    };
    let mut scene =
        include_str!("lower-body-walking-vr.mms").replace("assets/models/bisket.glb", asset);
    if validate {
        scene = scene.replace("XR.on()", "XR.off()");
    }
    let mut universe = engine::Universe::new(engine::ecs::World::default());
    universe
        .load_mms_source_at_path(&scene, "examples/lower-body-walking-vr.mms")
        .expect("load walking scene with live XR settings");
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );
    if validate {
        universe.systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.systems.skinned_mesh,
            &mut universe.systems.renderable,
            &mut universe.command_queue,
            0.0,
        );
        universe.systems.process_commands(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.render_assets,
            &mut universe.command_queue,
        );
        let avc = universe
            .world
            .all_components()
            .find(|&id| {
                universe
                    .world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .is_some_and(|c| c.walking_enabled)
            })
            .expect("walking AVC");
        assert!(universe.world.all_components().any(|id| {
            universe
                .world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some()
        }));
        assert!(
            universe
                .world
                .all_components()
                .any(|id| universe.world.component_label(id) == Some("walking_xr_settings"))
        );
        assert!(
            universe
                .world
                .all_components()
                .any(|id| universe.world.component_label(id) == Some("xr_floor_correction_slider"))
        );
        let gltf = universe
            .systems
            .humanoid_bone_map
            .request_for_avc(&mut universe.world, avc)
            .expect("avatar GLTF");
        assert!(
            universe
                .systems
                .humanoid_bone_map
                .report(gltf)
                .unwrap()
                .lower_body_ready()
        );
        println!("[walking] {model} smoke scene and mapped legs validated");
        return;
    }
    println!(
        "[walking] {model}: left stick at 0.6 m/s; use the full-height mirror to check planting, stops, strafing and crouching"
    );
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
