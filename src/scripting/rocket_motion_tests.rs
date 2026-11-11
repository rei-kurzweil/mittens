use crate::engine::graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader};
#[derive(Default)]
struct Uploader(u32);
impl MeshUploader for Uploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}
impl TextureUploader for Uploader {
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
}
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
#[derive(Default)]
struct TestClock(AtomicU64);
impl crate::engine::ecs::system::ClockDriver for TestClock {
    fn name(&self) -> &'static str {
        "rocket-test"
    }
    fn time_now_sec(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64 / 120.0
    }
}
fn advance(scene: &mut crate::engine::Universe, clock: &TestClock) {
    clock.0.fetch_add(1, Ordering::Relaxed);
    scene.update(1.0 / 120.0, &Default::default());
}
use crate::engine::ecs::World;
use crate::engine::ecs::component::VelocityComponent;

fn has(world: &World, name: &str) -> bool {
    world
        .all_components()
        .any(|id| world.component_label(id) == Some(name))
}

#[test]
fn rocket_native_flight_hits_thin_wall_and_spawns_explosion_without_frame_bindings() {
    let mut scene = crate::engine::Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { rocket } from "../assets/components/projectiles/rocket.mms"
        T.position(0,0,-2).scale(4,4,0.02) {
            Destroyable.threshold(1).cell_size(0.1).chunk_size(0.8)
            R.cube()
            Zone.cube([0.5,0.5,0.5]) { Collidable.static() {} }
        }
        rocket({pose={translation=[0,0,0] rotation_quat_xyzw=[0,0,0,1] scale=[1,1,1]} source_tool=null source_user=null})
    "#, "examples/_rocket_native_test.mms").unwrap();
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    let mut uploader = Uploader::default();
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut uploader,
        &mut scene.command_queue,
    );
    let mut explosion = false;
    for _ in 0..80 {
        advance(&mut scene, &clock);
        explosion |= has(&scene.world, "rocket_explosion");
    }
    assert!(
        explosion,
        "impact must deliver the native zone event to MMS"
    );
    assert!(
        !has(&scene.world, "sta_rocket_flight"),
        "impact removes projectile"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while scene.systems.destroyable.cuts == 0 && std::time::Instant::now() < deadline {
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut uploader,
            &mut scene.command_queue,
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        scene.systems.destroyable.cuts > 0,
        "damage must produce a completed wall cut"
    );
    assert!(scene.world.all_components().any(|id| scene.world.get_component_by_id_as::<crate::engine::ecs::component::destroyable::DestroyedSourceComponent>(id).is_some()), "impact must cut the destructible wall");
}

#[test]
fn rocket_native_acceleration_caps_speed_and_expires_in_empty_space() {
    let mut scene = crate::engine::Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { rocket } from "../assets/components/projectiles/rocket.mms"
        rocket({pose={translation=[0,0,0] rotation_quat_xyzw=[0,0.70710678,0,0.70710678] scale=[1,1,1]} source_tool=null source_user=null})
    "#, "examples/_rocket_cruise_test.mms").unwrap();
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    for _ in 0..240 {
        advance(&mut scene, &clock);
    }
    let speeds: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| scene.world.get_component_by_id_as::<VelocityComponent>(id))
        .map(|v| v.linear_local_mps)
        .collect();
    assert_eq!(speeds.len(), 1);
    assert!((speeds[0][2] + 18.0).abs() < 1e-4, "{speeds:?}");
    let body = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("sta_rocket_body"))
        .unwrap();
    let position =
        crate::engine::ecs::system::TransformSystem::world_position(&scene.world, body).unwrap();
    assert!(
        position[0] < -20.0 && position[2].abs() < 0.01,
        "local acceleration follows launch rotation: {position:?}"
    );
    for _ in 0..520 {
        advance(&mut scene, &clock);
    }
    assert!(!has(&scene.world, "sta_rocket_flight"));
    assert!(!has(&scene.world, "rocket_explosion"));
}

#[test]
fn rocket_device_delayed_launch_runs_through_live_callback() {
    let mut scene = crate::engine::Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
        import { sta_rocket_launcher } from "../assets/components/sta-rocket-launcher.mms"
        let device = sta_rocket_launcher({name="test_launcher"})
        device.root
        on_global("DataEvent", fn(event) {
            if event == "FireTest" { device.use_start({user=null}) }
        })
    "#,
            "examples/_rocket_device_test.mms",
        )
        .unwrap();
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    use crate::engine::ecs::SignalEmitter;
    scene.systems.rx.push_event(
        Default::default(),
        crate::engine::ecs::EventSignal::DataEvent {
            name: "FireTest".into(),
            payload: None,
        },
    );
    for _ in 0..30 {
        advance(&mut scene, &clock);
    }
    assert!(has(&scene.world, "sta_rocket_flight"));
}

#[test]
fn rocket_sakura_held_trigger_launches_through_scene_handlers() {
    use crate::engine::ecs::{ComponentId, EventSignal, KeyboardEvent, SignalEmitter};
    let mut scene = crate::engine::Universe::new(World::default());
    let source = include_str!("../../examples/sakura-drone-conversion.mms")
        .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
    scene
        .load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")
        .unwrap();
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    for _ in 0..2 {
        advance(&mut scene, &clock);
    }
    let grip = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("sakura_sta_launcher_placement"))
        .unwrap();
    scene.systems.rx.push_event(
        grip,
        EventSignal::GrabStart {
            pointer: ComponentId::default(),
            target: grip,
            raycaster: ComponentId::default(),
            renderable: grip,
            ray_origin_world: [0.0; 3],
            ray_dir_world: [0.0, 0.0, -1.0],
        },
    );
    advance(&mut scene, &clock);
    scene.systems.rx.push_event(
        ComponentId::default(),
        EventSignal::KeyDown(KeyboardEvent {
            code: Some("Space".into()),
            key: " ".into(),
        }),
    );
    for _ in 0..30 {
        advance(&mut scene, &clock);
    }
    assert!(has(&scene.world, "sta_rocket_flight") || has(&scene.world, "rocket_explosion"));
}

#[test]
fn launcher_scene_ab_preserves_desktop_camera_and_renderables() {
    use crate::engine::graphics::CameraTarget;
    let source = include_str!("../../examples/sakura-drone-conversion.mms")
        .replace(
            "let desktop_view_enabled = false",
            "let desktop_view_enabled = true",
        )
        .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
    for launcher in [false, true] {
        let mut scene = crate::engine::Universe::new(World::default());
        let text = if launcher {
            source.as_str()
        } else {
            source
                .split("// The GLB's geometry is offset")
                .next()
                .unwrap()
        };
        scene
            .load_mms_source_at_path(text, "examples/sakura-drone-conversion.mms")
            .unwrap();
        for _ in 0..2 {
            scene.update(1.0 / 120.0, &Default::default());
        }
        for id in scene.world.all_components() {
            let record = scene.world.get_component_record(id).unwrap();
            if format!("{id:?}").eq("ComponentId(32v1)")
                || format!("{id:?}").eq("ComponentId(34v1)")
                || format!("{id:?}").eq("ComponentId(38v1)")
            {
                eprintln!(
                    "[launcher-ab] launcher={launcher} {id:?} type={} label={:?}",
                    record.component.name(),
                    scene.world.component_label(id)
                );
            }
        }
        let mut uploader = Uploader::default();
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut uploader,
            &mut scene.command_queue,
        );
        eprintln!(
            "[launcher-ab] launcher={launcher} instances={} cameras={}",
            scene.visuals.instance_count(),
            scene.visuals.visual_cameras().len()
        );
        assert!(scene.visuals.visual_camera(CameraTarget::Window).is_some());
        assert!(scene.visuals.instance_count() > 100);
    }
}

#[test]
fn sakura_road_cars_remain_at_street_level() {
    use crate::engine::ecs::system::TransformSystem;
    for (source, path) in [
        (
            include_str!("../../examples/sakura-drone-conversion.mms"),
            "examples/sakura-drone-conversion.mms",
        ),
        (
            include_str!("../../examples/sakura-drone-conversion-desktop.mms"),
            "examples/sakura-drone-conversion-desktop.mms",
        ),
    ] {
        let mut scene = crate::engine::Universe::new(World::default());
        let source = source.replace("let xr_view_enabled = true", "let xr_view_enabled = false");
        scene.load_mms_source_at_path(&source, path).unwrap();
        let clock = Arc::new(TestClock::default());
        scene.systems.clock.set_driver(clock.clone());
        let mut uploader = Uploader::default();
        for name in ["sakura_road_car_north", "sakura_road_car_south"] {
            let id = scene
                .world
                .all_components()
                .find(|&id| scene.world.component_label(id) == Some(name))
                .unwrap();
            let t = scene
                .world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(id)
                .unwrap();
            eprintln!(
                "[car-placement] BEFORE {path} {name} local={:?} world={:?} parent={:?}",
                t.translation(),
                TransformSystem::world_position(&scene.world, id),
                scene.world.parent_of(id)
            );
        }
        for step in 0..40 {
            advance(&mut scene, &clock);
            if step == 1 {
                scene.systems.prepare_render(
                    &mut scene.world,
                    &mut scene.visuals,
                    &mut scene.render_assets,
                    &mut uploader,
                    &mut scene.command_queue,
                );
            }
            if step == 0 || step == 39 {
                for name in ["sakura_road_car_north", "sakura_road_car_south"] {
                    let id = scene
                        .world
                        .all_components()
                        .find(|&id| scene.world.component_label(id) == Some(name))
                        .unwrap();
                    let position = TransformSystem::world_position(&scene.world, id).unwrap();
                    eprintln!("[car-placement] {path} step={step} {name}: {position:?}");
                    let expected = if name.ends_with("north") {
                        [-3.4, -17.0]
                    } else {
                        [3.4, 19.0]
                    };
                    assert!(
                        position[1] < 1.0
                            && (position[0] - expected[0]).abs() < 0.1
                            && (position[2] - expected[1]).abs() < 0.1,
                        "{path} {name}: {position:?}"
                    );
                }
            }
        }
    }
}
