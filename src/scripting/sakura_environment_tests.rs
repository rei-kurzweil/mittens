use super::rocket_motion_tests::{TestClock, Uploader, advance};
use crate::engine::Universe;
use crate::engine::ecs::component::material::MaterialComponent;
use crate::engine::ecs::component::{
    CollidableComponent, CollidableMode, EmissiveComponent, SliderComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::climbing_system::climbable_owner_for_hit;
use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;

fn load(source: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/_sakura_environment_test.mms")
        .unwrap();
    scene
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn prepare(scene: &mut Universe) {
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut Uploader::default(),
        &mut scene.command_queue,
    );
}

#[test]
fn sakura_towers_keep_xr_grip_surfaces_after_batching_and_pulse_their_beacons() {
    let mut scene = load(
        r#"
        import { sakura_twilight_street } from "../assets/components/buildings/sakura_twilight_street.mms"
        Clock.bpm(120)
        let material = Material(MaterialDefinition.anime())
        sakura_twilight_street(-18.0, 9.6, -0.4, material, material, material)
        T.position(151.35, -0.23, -22.23) {
            name = "tower_climber"
            InputXR.on() {
                T { CXR {} }
                XRHand(true, "Left", "GripAim") { T { Pointer {} } }
            }
        }
    "#,
    );
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    let tower = named(&scene, "sakura_twilight_radio_tower_1");
    let section = scene
        .world
        .find_component(tower, "#radio_tower_section_0")
        .unwrap();
    for index in 0..3 {
        let child = scene
            .world
            .find_component(tower, &format!("#radio_tower_section_{index}"))
            .unwrap();
        let collider = scene.world.find_component(child, "collidable").unwrap();
        assert_eq!(
            scene
                .world
                .get_component_by_id_as::<CollidableComponent>(collider)
                .unwrap()
                .mode,
            CollidableMode::Static,
        );
    }
    let pose = |y| openxr::Posef {
        position: openxr::Vector3f { x: 0.0, y, z: 0.0 },
        ..openxr::Posef::IDENTITY
    };
    let view = openxr::View {
        pose: pose(1.65),
        fov: openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    let mut frame = ExternalXrFrame {
        views: Some([view, view]),
        aim: [Some(pose(1.0)), None],
        grip: [Some(pose(1.0)), None],
        ..Default::default()
    };
    scene.systems.xr.set_external_frame(Some(frame));
    advance(&mut scene, &clock);
    prepare(&mut scene);
    advance(&mut scene, &clock);

    let hit = scene
        .systems
        .bvh
        .raycast_renderables([151.35, 1.0, -22.23], [0.0, 0.0, -1.0], 0.15)
        .unwrap();
    assert_eq!(climbable_owner_for_hit(&scene.world, hit.0), Some(section));
    let climber = named(&scene, "tower_climber");
    let start = TransformSystem::world_position(&scene.world, climber).unwrap()[1];
    frame.input.grip_pressed[0] = true;
    frame.input.grip_down[0] = true;
    scene.systems.xr.set_external_frame(Some(frame));
    advance(&mut scene, &clock);
    frame.input.grip_pressed[0] = false;
    frame.aim[0] = Some(pose(0.8));
    frame.grip[0] = Some(pose(0.8));
    scene.systems.xr.set_external_frame(Some(frame));
    advance(&mut scene, &clock);
    let moved = TransformSystem::world_position(&scene.world, climber).unwrap()[1];
    assert!(
        (moved - start - 0.2).abs() < 1e-4,
        "climber moved from {start} to {moved}"
    );

    let intensities = |scene: &Universe| {
        [0, 1, 2].map(|index| {
            let beacon = scene
                .world
                .find_component(tower, &format!("#radio_tower_beacon_{index}"))
                .unwrap();
            let glow = scene.world.find_component(beacon, "emissive").unwrap();
            scene
                .world
                .get_component_by_id_as::<EmissiveComponent>(glow)
                .unwrap()
                .intensity
        })
    };
    assert_eq!(intensities(&scene), [3.8; 3]);
    for _ in 0..49 {
        advance(&mut scene, &clock);
    }
    assert_eq!(intensities(&scene), [0.0; 3]);
    for _ in 0..210 {
        advance(&mut scene, &clock);
    }
    assert_eq!(intensities(&scene), [3.8; 3]);
}

const LIGHTING_FIXTURE: &str = r#"
    import { sakura_twilight_lighting } from "../assets/components/buildings/sakura_twilight_lighting.mms"
    let fixture_material = Material(MaterialDefinition.anime())
"#;

fn sky(scene: &Universe) -> &MaterialComponent {
    let surface = named(scene, "kawaii_twilight_sky_surface");
    let material = scene.world.find_component(surface, "material").unwrap();
    scene
        .world
        .get_component_by_id_as::<MaterialComponent>(material)
        .unwrap()
}

fn set_slider(scene: &mut Universe, value: f32) {
    let slider = named(scene, "sakura_twilight_time_slider");
    scene.command_queue.push_intent_now(
        slider,
        IntentValue::SliderSet {
            component_id: slider,
            value,
            emit_changed: true,
        },
    );
}

fn slider_value(scene: &Universe) -> f32 {
    scene
        .world
        .get_component_by_id_as::<SliderComponent>(named(scene, "sakura_twilight_time_slider"))
        .unwrap()
        .value()
}

fn tick(scene: &mut Universe) {
    scene.update(1.0 / 60.0, &Default::default());
}

#[test]
fn sakura_lighting_defaults_and_restores_the_original_evening_sky() {
    let mut scene = load(&format!(
        "{LIGHTING_FIXTURE}\n{}",
        r#"
        let lighting = sakura_twilight_lighting(1, fixture_material)
        lighting.world
        lighting.pedestal
    "#
    ));
    tick(&mut scene);
    let evening = sky(&scene).values.clone();
    assert_eq!(evening["low_color"], [0.93, 0.80, 0.66]);
    assert_eq!(evening["middle_color"], [0.68, 0.56, 0.84]);
    assert_eq!(evening["high_color"], [0.22, 0.17, 0.36]);
    assert_eq!(evening["moon_strength"], [1.2]);
    assert_eq!(scene.visuals.ambient_light(), [0.26, 0.24, 0.32]);
    assert_eq!(slider_value(&scene), 0.0);
    set_slider(&mut scene, 1.0);
    for _ in 0..3 {
        tick(&mut scene);
    }
    assert_eq!(sky(&scene).values["moon_strength"], [0.0]);
    assert_eq!(scene.visuals.ambient_light(), [0.54, 0.58, 0.66]);
    assert!(
        scene
            .visuals
            .lights()
            .iter()
            .any(|light| { light.color == [1.0, 0.93, 0.78] && light.intensity == 1.8 })
    );
    set_slider(&mut scene, 0.0);
    for _ in 0..3 {
        tick(&mut scene);
    }
    assert_eq!(sky(&scene).values, evening);
    assert_eq!(scene.visuals.ambient_light(), [0.26, 0.24, 0.32]);
    assert!(
        scene
            .visuals
            .lights()
            .iter()
            .any(|light| { light.color == [1.0, 0.86, 0.70] && light.intensity == 1.3 })
    );
}

#[test]
fn sakura_environment_udp_sends_late_join_state_and_slider_edits_in_both_directions() {
    fn peer(rank: u8, local: u16, remote: u16) -> Universe {
        load(&format!(
            r#"
            {LIGHTING_FIXTURE}
            let lighting = sakura_twilight_lighting({rank}, fixture_material)
            lighting.world
            lighting.pedestal
            let socket = UdpSocket.bind("127.0.0.1:{local}") {{}}
            socket
            let channels = T {{}}
            channels
            lighting.connect(socket, channels, "127.0.0.1:{remote}")
            on_global("FrameTick", fn(event) {{ lighting.send() }})
        "#
        ))
    }
    let ports = [0, 1].map(|_| UdpSocket::bind("127.0.0.1:0").unwrap());
    let [host_port, client_port] = ports
        .each_ref()
        .map(|socket| socket.local_addr().unwrap().port());
    drop(ports);
    let mut host = peer(1, host_port, client_port);
    set_slider(&mut host, 1.0);
    for _ in 0..10 {
        tick(&mut host);
    }
    let mut client = peer(2, client_port, host_port);
    let wait_for_value = |host: &mut Universe, client: &mut Universe, value: f32| {
        for _ in 0..250 {
            tick(host);
            tick(client);
            if (slider_value(host) - value).abs() < 1e-4
                && (slider_value(client) - value).abs() < 1e-4
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        panic!(
            "expected {value}, host={}, client={}",
            slider_value(host),
            slider_value(client)
        );
    };
    wait_for_value(&mut host, &mut client, 1.0_f32);
    assert_eq!(sky(&client).values["moon_strength"], [0.0]);
    set_slider(&mut client, 0.25);
    wait_for_value(&mut host, &mut client, 0.25);
    assert!((sky(&host).values["moon_strength"][0] - 0.9).abs() < 1e-4);
    set_slider(&mut host, 0.0);
    wait_for_value(&mut host, &mut client, 0.0);
    assert_eq!(sky(&client).values["moon_strength"], [1.2]);
}
