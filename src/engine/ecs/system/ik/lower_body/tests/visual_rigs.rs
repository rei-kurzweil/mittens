use super::*;

#[test]
fn collision_disabled_visual_rig_solves_support_without_moving_root() {
    let mut f = Fixture::new(None);
    {
        let avatar = f
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap();
        avatar.collision_enabled = false;
        avatar.capsule_ready = false;
        avatar.capsule_transform_id = None;
        avatar.capsule_collidable_id = None;
    }
    let input_before = matrix(&f.world, f.driven).unwrap();
    let root_before = matrix(&f.world, f.root).unwrap();
    f.step();
    for foot in f.feet() {
        assert!(foot.support.is_some());
        assert!(foot.solved_sole[1].abs() < 1e-4, "{foot:?}");
    }
    f.assert_pose();
    assert_eq!(matrix(&f.world, f.driven).unwrap(), input_before);
    assert_eq!(matrix(&f.world, f.root).unwrap(), root_before);
}

#[test]
fn collision_disabled_visual_rig_does_not_invent_missing_support() {
    let mut f = Fixture::new(None);
    let avatar = f
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap();
    avatar.collision_enabled = false;
    avatar.capsule_ready = false;
    avatar.capsule_transform_id = None;
    avatar.capsule_collidable_id = None;
    f.world
        .get_component_by_id_as_mut::<ZoneComponent>(f.floor_zone)
        .unwrap()
        .enabled = false;
    f.propagate();
    let root_before = matrix(&f.world, f.root).unwrap();
    f.step();
    assert!(f.feet().iter().all(|foot| foot.support.is_none()));
    assert_eq!(matrix(&f.world, f.root).unwrap(), root_before);
    f.assert_pose();
}

#[test]
fn collision_disabled_visual_rig_retains_support_during_crouch() {
    for scale in [0.5, 1., 1.7] {
        let mut f = Fixture::new(None);
        let frame = &mut f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(f.scene)
            .unwrap()
            .transform;
        frame.scale = [scale; 3];
        frame.recompute_model();
        let avatar = f
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap();
        avatar.collision_enabled = false;
        avatar.capsule_ready = false;
        avatar.capsule_transform_id = None;
        avatar.capsule_collidable_id = None;
        for drop in [0., 0.1, 0.2, 0.1, 0.] {
            f.drop_body(drop);
            let input = matrix(&f.world, f.driven).unwrap();
            let root = matrix(&f.world, f.root).unwrap();
            f.step();
            for foot in f.feet() {
                assert!(foot.support.is_some(), "crouch={drop}: {foot:?}");
                assert!(foot.solved_sole[1].abs() < 1e-4, "crouch={drop}: {foot:?}");
            }
            assert_eq!(matrix(&f.world, f.driven).unwrap(), input);
            assert_eq!(matrix(&f.world, f.root).unwrap(), root);
        }
        // Previously acquired support must still disappear when the floor does.
        f.world
            .get_component_by_id_as_mut::<ZoneComponent>(f.floor_zone)
            .unwrap()
            .enabled = false;
        f.propagate();
        f.step();
        assert!(f.feet().iter().all(|foot| foot.support.is_none()));
    }
}

#[test]
fn collision_disabled_support_reference_follows_locomotion_height() {
    let mut f = Fixture::new(None);
    let locomotion = f.world.add_component(TransformComponent::new());
    f.world.add_child(f.scene, locomotion).unwrap();
    f.world.set_parent(f.input, Some(locomotion)).unwrap();
    let avatar = f
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap();
    avatar.collision_enabled = false;
    avatar.capsule_ready = false;
    f.propagate();
    f.step();
    assert!(f.feet().iter().all(|foot| foot.support.is_some()));
    let frame = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(locomotion)
        .unwrap()
        .transform;
    frame.translation[1] = 0.4;
    frame.recompute_model();
    f.propagate();
    f.step();
    assert!(
        f.feet().iter().all(|foot| foot.support.is_none()),
        "old floor must not anchor a translated player"
    );
}
