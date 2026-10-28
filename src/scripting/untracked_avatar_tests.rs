use crate::engine::Universe;
use crate::engine::ecs::component::{
    AvatarControlComponent, TransformComponent, VelocityComponent,
};
use crate::engine::ecs::system::ik::lower_body::{FootPhase, LowerBodyStatus};
use crate::engine::ecs::{ComponentId, World};

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}

#[test]
fn untracked_lumae_keep_authored_heads_and_walk_from_velocity_and_scripted_ancestor_motion() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
        import { lumae_avatar } from "../assets/components/characters/lumae/avatar.mms"
        let material = Material(MaterialDefinition.anime())
        T.position(0, -0.1, 0) { Zone.cube([20, 0.1, 20]) { Collidable.static() {} } }
        Velocity { name = "npc_velocity"
            T.position(-2, 0, 0) { name = "moving_npc" lumae_avatar("velocity_lumae", material) }
        }
        T.position(2, 0, 0) { name = "scripted_ancestor"
            T { lumae_avatar("scripted_lumae", material) }
        }
    "#,
            "examples/_untracked-lumae-test.mms",
        )
        .unwrap();
    for _ in 0..12 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let actors = [
        named(&scene, "velocity_lumae_avc"),
        named(&scene, "scripted_lumae_avc"),
    ];
    let roots = actors.map(|id| {
        let avatar = scene
            .world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .unwrap();
        assert!(
            avatar.head_mount.is_none() && avatar.displaced_head.is_none(),
            "NPC received head embodiment"
        );
        assert!(avatar.capsule_ready, "NPC capsule not ready");
        assert!(
            matches!(
                scene.systems.lower_body_ik.status(id),
                LowerBodyStatus::Ready | LowerBodyStatus::Limited
            ),
            "{:?}",
            scene.systems.lower_body_ik.status(id)
        );
        let model = avatar.model_root_id.unwrap();
        assert_eq!(
            scene
                .world
                .get_component_by_id_as::<TransformComponent>(model)
                .unwrap()
                .transform
                .translation,
            [0.0; 3]
        );
        model
    });
    let heads = roots.map(|root| {
        scene
            .world
            .find_component(root, "[name='J_Bip_C_Head']")
            .unwrap()
    });
    for head in heads {
        assert_eq!(
            scene
                .world
                .component_label(scene.world.parent_of(head).unwrap()),
            Some("J_Bip_C_Neck")
        );
    }
    let velocity = named(&scene, "npc_velocity");
    scene
        .world
        .get_component_by_id_as_mut::<VelocityComponent>(velocity)
        .unwrap()
        .linear_local_mps = [0.0, 0.0, 0.3];
    let ancestor = named(&scene, "scripted_ancestor");
    let mut saw_stance = [false; 2];
    for _ in 0..180 {
        let t = &mut scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(ancestor)
            .unwrap()
            .transform;
        t.translation[0] += 0.005;
        t.recompute_model();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, ancestor);
        scene.update(1.0 / 60.0, &Default::default());
        for (i, avc) in actors.iter().enumerate() {
            let feet = scene
                .systems
                .lower_body_ik
                .feet(*avc)
                .expect("NPC feet must stay bound");
            saw_stance[i] |= feet.iter().any(|f| f.phase == FootPhase::Planted);
            assert!(
                feet.iter()
                    .all(|f| f.solved_sole.iter().all(|v| v.is_finite()))
            );
        }
    }
    for (i, id) in actors.into_iter().enumerate() {
        let gait = scene.systems.lower_body_ik.walking(id).unwrap();
        assert!(
            gait.steps_started.iter().all(|steps| *steps > 0),
            "no alternating gait: {gait:?}"
        );
        assert!(saw_stance[i]);
        assert_eq!(
            scene
                .world
                .get_component_by_id_as::<TransformComponent>(roots[i])
                .unwrap()
                .transform
                .translation,
            [0.0; 3]
        );
        assert_eq!(
            scene
                .world
                .component_label(scene.world.parent_of(heads[i]).unwrap()),
            Some("J_Bip_C_Neck")
        );
    }
    scene
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(actors[0])
        .unwrap()
        .walking_enabled = false;
    scene.update(1.0 / 60.0, &Default::default());
    assert!(
        scene
            .systems
            .lower_body_ik
            .feet(actors[0])
            .unwrap()
            .iter()
            .all(|f| f.phase == FootPhase::Rest)
    );
    assert!(
        scene
            .systems
            .lower_body_ik
            .walking(actors[1])
            .unwrap()
            .steps_started
            .iter()
            .all(|n| *n > 0),
        "one avatar's opt-out must not reset the other"
    );
}

#[test]
fn untracked_lumae_mms_pacing_drives_the_shared_walking_solver() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { lumae_avatar } from "../assets/components/characters/lumae/avatar.mms"
        import { pacing_and_waiting_around } from "../assets/components/characters/lumae/behavior/pacing_and_waiting_around.mms"
        T.position(0, -0.1, 0) { Zone.cube([20, 0.1, 20]) { Collidable.static() {} } }
        let body = T { name = "paced_body" lumae_avatar("paced_lumae", Material(MaterialDefinition.anime())) }
        let pacing = pacing_and_waiting_around(body, { wait_seconds = 0.1 turn_seconds = 0.4 walk_seconds = 1.0 speed = 0.35 ease_seconds = 0.2 })
        pacing
        pacing.start()
    "#, "examples/_paced-lumae-test.mms").unwrap();
    for _ in 0..180 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let id = named(&scene, "paced_lumae_avc");
    let gait = scene.systems.lower_body_ik.walking(id).unwrap();
    assert!(
        gait.steps_started.iter().all(|n| *n > 0),
        "pacing did not cause alternating steps: {gait:?}"
    );
    let avatar = scene
        .world
        .get_component_by_id_as::<AvatarControlComponent>(id)
        .unwrap();
    assert!(avatar.head_mount.is_none());
    assert!(
        scene
            .systems
            .lower_body_ik
            .feet(id)
            .unwrap()
            .iter()
            .all(|f| f.support.is_some())
    );
}
