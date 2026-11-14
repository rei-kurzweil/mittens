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
