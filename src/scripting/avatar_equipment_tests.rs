use super::rocket_motion_tests::Uploader;
use crate::engine::Universe;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::transform::TransformSpace;

fn scene(source: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/_avatar_equipment_test.mms")
        .unwrap();
    step(&mut scene);
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut Uploader::default(),
        &mut scene.command_queue,
    );
    step(&mut scene);
    scene
}

fn step(scene: &mut Universe) {
    scene.update(0.0, &Default::default());
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn position(scene: &Universe, id: ComponentId) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, id).unwrap()
}

fn move_to(scene: &mut Universe, id: ComponentId, translation: [f32; 3]) {
    let mut value = TransformSystem::world_trs(&scene.world, id).unwrap();
    value.translation = translation;
    scene.command_queue.push_intent_now(
        id,
        IntentValue::SetTransformTrs {
            component_id: id,
            space: TransformSpace::World,
            value,
        },
    );
    step(scene);
}

fn grab(scene: &mut Universe, target: ComponentId) {
    let pointer = named(scene, "test_pointer");
    let renderable = scene.world.find_component(target, "renderable").unwrap();
    scene.command_queue.push_event(
        renderable,
        EventSignal::GrabStart {
            pointer,
            target,
            renderable,
            raycaster: pointer,
            ray_origin_world: [0.0, 0.0, 2.0],
            ray_dir_world: [0.0, 0.0, -1.0],
        },
    );
    step(scene);
    assert_eq!(scene.world.parent_of(target), Some(named(scene, "hand")));
}

fn release(scene: &mut Universe, target: ComponentId) {
    let pointer = named(scene, "test_pointer");
    scene
        .command_queue
        .push_event(target, EventSignal::GrabEnd { pointer, target });
    step(scene);
    step(scene);
}

fn drop_round_into(scene: &mut Universe, round: ComponentId, slot: ComponentId) -> ComponentId {
    let probe = scene
        .world
        .find_component(round, "[name='sta_rocket_ammo_zone']")
        .unwrap();
    grab(scene, round);
    let origin = position(scene, round);
    let probe_center = position(scene, probe);
    let slot_center = position(scene, slot);
    let translation = std::array::from_fn(|i| slot_center[i] + origin[i] - probe_center[i]);
    move_to(scene, round, translation);
    release(scene, round);
    probe
}

fn assert_round_hangs_at_slot(scene: &Universe, round: ComponentId, slot: ComponentId) {
    assert_eq!(scene.world.parent_of(round), Some(slot));
    let pose = TransformSystem::world_trs(&scene.world, round).unwrap();
    for (actual, expected) in pose.translation.into_iter().zip(position(scene, slot)) {
        assert!((actual - expected).abs() < 1e-5);
    }
    let nose = crate::utils::math::quat_rotate_vec3(pose.rotation_quat_xyzw, [0.0, 0.0, -1.0]);
    assert!(nose[1] < -0.98, "round points down at rest: {nose:?}");
    for scale in pose.scale {
        assert!((scale - 1.0).abs() < 1e-5);
    }
}

#[test]
fn equipped_sta_round_hangs_in_its_slot_and_cannot_push_a_body() {
    let mut scene = scene(
        r#"
        import { mitten_model } from "../assets/components/characters/mitten.mms"
        import { sta_rocket_ammo } from "../assets/components/projectiles/sta_rocket_ammo.mms"
        let rig = mitten_model("mitten", {})
        T { rig.model }
        T { name = "hand" Pointer { name = "test_pointer" } }
        T.position(2, 0, 0) { sta_rocket_ammo("first_round") sta_rocket_ammo("second_round") }
        let body = T.position(4, 0, 0) { name = "body" }
        let contact = Zone.sphere(0.3) { Collidable.slide().movement_target(body) }
        body.attach(contact)
        body
    "#,
    );
    let slot = named(&scene, "equipment_left_thigh");
    let round = named(&scene, "first_round");
    let probe = drop_round_into(&mut scene, round, slot);
    assert_round_hangs_at_slot(&scene, round, slot);

    let other = named(&scene, "second_round");
    drop_round_into(&mut scene, other, slot);
    assert_ne!(scene.world.parent_of(other), Some(slot));
    move_to(&mut scene, other, [2.0, 0.0, 0.0]);

    let body = named(&scene, "body");
    let probe_center = position(&scene, probe);
    let overlap = [probe_center[0] + 0.05, probe_center[1], probe_center[2]];
    move_to(&mut scene, body, overlap);
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    assert_eq!(position(&scene, body), overlap);
    assert_eq!(position(&scene, probe), probe_center);

    grab(&mut scene, round);
    move_to(&mut scene, round, [3.0, 0.0, 0.0]);
    release(&mut scene, round);
    assert_eq!(scene.world.parent_of(round), None);
    drop_round_into(&mut scene, other, slot);
    assert_round_hangs_at_slot(&scene, other, slot);
}

#[test]
fn mitten_equipment_attaches_on_release_and_stays_grabbable() {
    let mut scene = scene(
        r#"
        import { mitten_model } from "../assets/components/characters/mitten.mms"
        import { detach_avatar_equipment } from "../assets/components/characters/avatar_equipment.mms"
        let rig = mitten_model("mitten", {})
        let avatar = T { name = "avatar" rig.model }
        avatar
        on_global("DataEvent", fn(event) {
            if event == "remove_avatar" {
                detach_avatar_equipment(rig.model)
                avatar.remove_subtree()
            }
        })
        T { name = "hand" Pointer { name = "test_pointer" } }
        T.position(3, 0, 0).rotation(0.0, 0.4, 0.0).scale(0.2, 0.3, 0.4) {
            name = "plain"
            Grabbable {}
            R.cube()
        }
        T.position(3, 0, 0) {
            name = "shaped"
            Grabbable.attachment_index(0) {}
            R.cube()
            T.position(-0.3, 0, 0) { Zone.sphere(0.04) {} }
        }
        T.position(3, 0, 0) {
            name = "higher"
            Grabbable.attachment_index(1) {}
            R.cube()
        }
    "#,
    );
    let avatar = named(&scene, "avatar");
    let hand = named(&scene, "hand");
    let left = named(&scene, "equipment_left_thigh");
    let right = named(&scene, "equipment_right_thigh");
    let back = named(&scene, "equipment_back");
    let plain = named(&scene, "plain");
    let shaped = named(&scene, "shaped");
    let higher = named(&scene, "higher");
    let [x, y, z] = position(&scene, left);
    assert!(x > 0.18 && position(&scene, right)[0] < -0.18);
    assert!(position(&scene, back)[2] < -0.16);

    grab(&mut scene, plain);
    move_to(&mut scene, plain, [x, y, z]);
    for _ in 0..3 {
        step(&mut scene);
    }
    assert_eq!(scene.world.parent_of(plain), Some(hand));
    let before = TransformSystem::world_trs(&scene.world, plain).unwrap();
    release(&mut scene, plain);
    assert_eq!(scene.world.parent_of(plain), Some(left));
    let after = TransformSystem::world_trs(&scene.world, plain).unwrap();
    let rotation_dot = before
        .rotation_quat_xyzw
        .into_iter()
        .zip(after.rotation_quat_xyzw)
        .map(|(a, b)| a * b)
        .sum::<f32>();
    assert!(rotation_dot.abs() > 1.0 - 1e-5);
    for (old, new) in before
        .translation
        .into_iter()
        .chain(before.scale)
        .zip(after.translation.into_iter().chain(after.scale))
    {
        assert!((old - new).abs() < 1e-5);
    }
    move_to(&mut scene, avatar, [2.0, 0.0, 0.0]);
    assert!((position(&scene, plain)[0] - x - 2.0).abs() < 1e-5);

    grab(&mut scene, plain);
    move_to(&mut scene, plain, [4.0, 0.0, 0.0]);
    release(&mut scene, plain);
    assert_eq!(scene.world.parent_of(plain), None);

    let [x, y, z] = position(&scene, right);
    grab(&mut scene, higher);
    move_to(&mut scene, higher, [x, y, z]);
    release(&mut scene, higher);
    assert_eq!(scene.world.parent_of(higher), None);

    grab(&mut scene, shaped);
    move_to(&mut scene, shaped, [x + 0.3, y, z]);
    release(&mut scene, shaped);
    assert_eq!(scene.world.parent_of(shaped), Some(right));
    let pose = position(&scene, shaped);
    scene.command_queue.push_event(
        avatar,
        EventSignal::DataEvent {
            name: "remove_avatar".into(),
            payload: None,
        },
    );
    step(&mut scene);
    step(&mut scene);
    assert_eq!(scene.world.parent_of(shaped), None);
    assert!(scene.world.get_component_record(avatar).is_none());
    assert_eq!(position(&scene, shaped), pose);
}

#[test]
fn miku_equipment_zones_follow_the_scaled_rig() {
    const MODEL: &str = "assets/models/local/third_party/TDA_Hatsune_Miku/tda_miku_with_bones.glb";
    if !std::path::Path::new(MODEL).is_file() {
        return;
    }
    let mut scene = scene(&format!(
        r#"
        import {{ install_miku_equipment }} from "../assets/components/characters/tda_miku_rig.mms"
        import {{ sta_rocket_ammo }} from "../assets/components/projectiles/sta_rocket_ammo.mms"
        let model = GLTF.new("{MODEL}") {{}}
        T.scale(0.09,0.09,0.09) {{ model }}
        install_miku_equipment(model)
        T {{ name = "hand" Pointer {{ name = "test_pointer" }} }}
        T.position(3, 0, 0).scale(0.1, 0.2, 0.3) {{
            name = "prop"
            Grabbable.attachment_index(-1) {{}}
            R.cube()
        }}
        T.position(2, 0, 0) {{ sta_rocket_ammo("round") }}
    "#
    ));
    let left = position(&scene, named(&scene, "equipment_left_thigh"));
    let right = position(&scene, named(&scene, "equipment_right_thigh"));
    let back_zone = named(&scene, "equipment_back");
    let back = position(&scene, back_zone);
    assert!(left[0] > 0.18 && right[0] < -0.18);
    assert!(back[1] > left[1] && back[2] < -0.12);
    let prop = named(&scene, "prop");
    grab(&mut scene, prop);
    move_to(&mut scene, prop, back);
    release(&mut scene, prop);
    assert_eq!(scene.world.parent_of(prop), Some(back_zone));
    let pose = TransformSystem::world_trs(&scene.world, prop).unwrap();
    for (actual, expected) in pose.scale.into_iter().zip([0.1, 0.2, 0.3]) {
        assert!((actual - expected).abs() < 1e-5);
    }
    let round = named(&scene, "round");
    for name in [
        "equipment_left_thigh",
        "equipment_right_thigh",
        "equipment_back",
    ] {
        let zone = named(&scene, name);
        drop_round_into(&mut scene, round, zone);
        assert_round_hangs_at_slot(&scene, round, zone);
    }
}
