use super::rig::{LowerBodyRig, matrix, point};
use super::*;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    AvatarControlComponent, HumanoidSlot, InputXRComponent, TransformComponent, ZoneComponent,
};
use crate::engine::ecs::component::{BoneRestPoseComponent, CollidableComponent, GLTFComponent};
use crate::engine::ecs::system::HumanoidBoneMapReport;
use crate::engine::ecs::system::humanoid_bone_map_system::{
    HumanoidSlotStatus, ResolvedHumanoidTarget, ResolvedHumanoidTargetKind,
};
use crate::engine::ecs::system::{
    HumanoidBoneMapSystem, HumanoidSlotProvenance, HumanoidSlotReport, SystemWorld,
};
use crate::engine::ecs::{CommandQueue, RxWorld};
use crate::engine::graphics::{RenderAssets, VisualWorld};
use crate::utils::math::quat_rotation_y;
use crate::utils::math::{vec3_len, vec3_sub};
use std::collections::BTreeMap;

struct Fixture {
    world: World,
    systems: SystemWorld,
    queue: CommandQueue,
    visuals: VisualWorld,
    assets: RenderAssets,
    scene: ComponentId,
    input: ComponentId,
    driven: ComponentId,
    avatar: ComponentId,
    root: ComponentId,
    capsule: ComponentId,
    floor: ComponentId,
    floor_zone: ComponentId,
    floor_collider: ComponentId,
    joints: [[ComponentId; 3]; 2],
    offsets: [[f32; 3]; 2],
    base_height: f32,
}
fn close(a: [f32; 3], b: [f32; 3]) {
    assert!(vec3_len(vec3_sub(a, b)) < 0.0001, "{a:?} != {b:?}");
}
fn named(
    world: &mut World,
    parent: ComponentId,
    name: &str,
    t: [f32; 3],
    r: [f32; 4],
    s: [f32; 3],
) -> ComponentId {
    let id = world.add_component_boxed_named(
        name,
        Box::new(
            TransformComponent::new()
                .with_position(t[0], t[1], t[2])
                .with_rotation_quat(r)
                .with_scale(s[0], s[1], s[2]),
        ),
    );
    world.add_child(parent, id).unwrap();
    let rest = world.add_component(BoneRestPoseComponent::new(t, r, s));
    world.add_child(id, rest).unwrap();
    id
}
impl Fixture {
    fn new(asset: Option<&str>) -> Self {
        let mut world = World::default();
        let scene = world.add_component(TransformComponent::new());
        let mut xr = InputXRComponent::on();
        xr.pose_valid = true;
        let input = world.add_component(xr);
        world.add_child(scene, input).unwrap();
        let driven = world.add_component(TransformComponent::new().with_position(0.0, 1.6, 0.0));
        world.add_child(input, driven).unwrap();
        let avatar = world.add_component(AvatarControlComponent::new());
        world.add_child(driven, avatar).unwrap();
        let root = world.add_component(TransformComponent::new().with_position(0.0, -1.6, 0.0));
        world.add_child(avatar, root).unwrap();
        let (report, offsets, base_height) = if let Some(asset) = asset {
            let document = gltf::Gltf::open(asset).unwrap();
            let gltf = world.add_component(GLTFComponent::new(asset));
            world.add_child(root, gltf).unwrap();
            let mut nodes = Vec::new();
            for node in document.nodes() {
                let (t, r, s) = node.transform().decomposed();
                nodes.push(named(
                    &mut world,
                    gltf,
                    node.name().unwrap_or("unnamed"),
                    t,
                    r,
                    s,
                ));
            }
            for node in document.nodes() {
                for child in node.children() {
                    world
                        .set_parent(nodes[child.index()], Some(nodes[node.index()]))
                        .unwrap();
                }
            }
            let joints: Vec<_> = document
                .skins()
                .flat_map(|s| s.joints())
                .map(|n| nodes[n.index()])
                .collect();
            let c = world
                .get_component_by_id_as_mut::<GLTFComponent>(gltf)
                .unwrap();
            c.spawned = true;
            c.spawned_node_transforms = nodes;
            c.armature_joint_transforms = joints;
            let mut maps = HumanoidBoneMapSystem::default();
            maps.resolve_gltf(&mut world, gltf, true);
            (
                maps.report(gltf).unwrap().clone(),
                [[0.0, 0.0530325, 0.0874534]; 2],
                0.02903146,
            )
        } else {
            let hips = named(
                &mut world,
                root,
                "renamed pelvis",
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
                [1.0; 3],
            );
            let mut slots = BTreeMap::new();
            let mut add = |slot, component| {
                slots.insert(
                    slot,
                    HumanoidSlotReport {
                        slot,
                        status: HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                            component,
                            kind: ResolvedHumanoidTargetKind::SkinJoint,
                        }),
                        provenance: HumanoidSlotProvenance::Explicit,
                        diagnostic: None,
                    },
                );
            };
            add(HumanoidSlot::Hips, hips);
            for (x, slots) in [
                (
                    -0.12,
                    [
                        HumanoidSlot::LeftUpperLeg,
                        HumanoidSlot::LeftLowerLeg,
                        HumanoidSlot::LeftFoot,
                    ],
                ),
                (
                    0.12,
                    [
                        HumanoidSlot::RightUpperLeg,
                        HumanoidSlot::RightLowerLeg,
                        HumanoidSlot::RightFoot,
                    ],
                ),
            ] {
                let upper = named(
                    &mut world,
                    hips,
                    "renamed thigh",
                    [x, 0.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                    [1.0; 3],
                );
                let knee = named(
                    &mut world,
                    upper,
                    "renamed knee",
                    [0.0, -0.4, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                    [1.0; 3],
                );
                let foot = named(
                    &mut world,
                    knee,
                    "renamed ankle",
                    [0.0, -0.5, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                    [1.0; 3],
                );
                for (slot, id) in slots.into_iter().zip([upper, knee, foot]) {
                    add(slot, id);
                }
            }
            (
                HumanoidBoneMapReport {
                    owning_gltf: root,
                    source: None,
                    generation: 1,
                    valid: true,
                    diagnostics: Vec::new(),
                    slots,
                },
                [[0.0, -0.1, 0.0]; 2],
                0.0,
            )
        };
        let binding = LowerBodyRig::bind(&world, root, &report).unwrap();
        let joints = binding.legs.each_ref().map(|l| l.joints);
        let capsule = world.add_component(TransformComponent::new().with_position(
            0.0,
            1.0 + base_height,
            0.0,
        ));
        world.add_child(scene, capsule).unwrap();
        let zone = world.add_component(ZoneComponent::capsule_y(0.2, 0.8));
        world.add_child(capsule, zone).unwrap();
        let collider = world.add_component(CollidableComponent::slide());
        world.add_child(zone, collider).unwrap();
        let floor = world.add_component(TransformComponent::new().with_position(
            0.0,
            base_height - 0.05,
            0.0,
        ));
        world.add_child(scene, floor).unwrap();
        let floor_zone = world.add_component(ZoneComponent::cube([3.0, 0.05, 3.0]));
        world.add_child(floor, floor_zone).unwrap();
        let floor_collider = world.add_component(CollidableComponent::static_());
        world.add_child(floor_zone, floor_collider).unwrap();
        let c = world
            .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
            .unwrap();
        c.model_root_id = Some(root);
        c.humanoid_map_generation = report.generation;
        c.crouch_sole_offsets = Some(offsets);
        // These fixtures isolate the original crouch policy. Walking tests
        // below restore the component's default to exercise automatic stepping.
        c.walking_enabled = false;
        c.capsule_transform_id = Some(capsule);
        c.capsule_collidable_id = Some(collider);
        c.capsule_ready = true;
        let mut systems = SystemWorld::default();
        systems.lower_body_ik.register(avatar);
        systems.lower_body_ik.avatars.get_mut(&avatar).unwrap().rig = Some(binding);
        let mut f = Self {
            world,
            systems,
            queue: CommandQueue::new(),
            visuals: VisualWorld::default(),
            assets: RenderAssets::new(),
            scene,
            input,
            driven,
            avatar,
            root,
            capsule,
            floor,
            floor_zone,
            floor_collider,
            joints,
            offsets,
            base_height,
        };
        f.propagate();
        f
    }
    fn propagate(&mut self) {
        self.systems
            .transform_changed(&mut self.world, &mut self.visuals, self.scene);
    }
    fn drop_body(&mut self, drop: f32) {
        let t = &mut self
            .world
            .get_component_by_id_as_mut::<TransformComponent>(self.root)
            .unwrap()
            .transform;
        t.translation[1] = -1.6 - drop;
        t.recompute_model();
        self.propagate();
        self.step();
    }
    fn step(&mut self) {
        self.step_with_dt(1.0 / 60.0);
    }
    fn step_with_dt(&mut self, dt: f32) {
        self.systems.lower_body_ik.tick(
            &mut self.world,
            &mut self.queue,
            &mut self.systems.static_contact,
            &self.systems.attachment,
            &self.systems.humanoid_bone_map,
            dt,
        );
        self.queue.flush(
            &mut self.world,
            &mut self.systems,
            &mut self.visuals,
            &mut self.assets,
        );
    }
    fn feet(&self) -> [FootState; 2] {
        *self.systems.lower_body_ik.feet(self.avatar).unwrap()
    }
    fn assert_pose(&self) {
        for (i, joints) in self.joints.iter().enumerate() {
            let state = self.feet()[i];
            let actual = point(matrix(&self.world, joints[2]).unwrap(), self.offsets[i]);
            close(actual, state.solved_sole);
            for joint in joints {
                let t = self
                    .world
                    .get_component_by_id_as::<TransformComponent>(*joint)
                    .unwrap();
                assert!(t.transform.rotation.into_iter().all(f32::is_finite));
            }
            let binding = self.systems.lower_body_ik.avatars[&self.avatar]
                .rig
                .as_ref()
                .unwrap();
            for j in 0..3 {
                let t = &self
                    .world
                    .get_component_by_id_as::<TransformComponent>(joints[j])
                    .unwrap()
                    .transform;
                close(t.translation, binding.legs[i].rest[j].translation);
                close(t.scale, binding.legs[i].rest[j].scale);
            }
        }
    }
}

mod walking;

#[test]
fn crouch_holds_soles_and_restores_standing_without_accumulation() {
    let mut f = Fixture::new(None);
    let driven = matrix(&f.world, f.driven).unwrap();
    let capsule = matrix(&f.world, f.capsule).unwrap();
    for drop in [0.0, 0.15, 0.4, 0.55, 0.25, 0.0] {
        f.drop_body(drop);
        f.assert_pose();
        assert_eq!(
            f.systems.lower_body_ik.status(f.avatar),
            LowerBodyStatus::Ready
        );
        for foot in f.feet() {
            assert!(foot.solved_sole[1].abs() < 0.0001);
            assert!(foot.residual < 0.0001);
        }
        assert_eq!(matrix(&f.world, f.driven).unwrap(), driven);
        assert_eq!(matrix(&f.world, f.capsule).unwrap(), capsule);
    }
    // Supported standing legs have one owner even if animation wrote them.
    for joint in f.joints[0] {
        let t = &mut f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(joint)
            .unwrap()
            .transform;
        t.rotation = quat_rotation_y(0.5);
        t.recompute_model();
    }
    f.propagate();
    f.step();
    f.assert_pose();
    f.drop_body(0.4);
    let feet = f.feet();
    let rebuilds = f.systems.static_contact.support_index(&f.world).rebuilds;
    for _ in 0..120 {
        f.step();
        f.assert_pose();
        for (a, b) in f.feet().into_iter().zip(feet) {
            close(a.solved_sole, b.solved_sole);
        }
    }
    for joints in f.joints {
        assert!(
            matrix(&f.world, joints[1]).unwrap()[3][2] > 0.1,
            "knees must bend forward"
        );
    }
    assert_eq!(
        f.systems.static_contact.support_index(&f.world).rebuilds,
        rebuilds
    );
    f.drop_body(0.0);
    let binding = f.systems.lower_body_ik.avatars[&f.avatar]
        .rig
        .as_ref()
        .unwrap();
    for leg in &binding.legs {
        for (id, rest) in leg.joints.into_iter().zip(leg.rest) {
            assert_eq!(
                f.world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .unwrap()
                    .transform
                    .rotation,
                rest.rotation_quat_xyzw
            );
        }
    }
}

#[test]
fn crouch_real_reimu_and_bisket_maps_solve_with_nonidentity_bind_frames() {
    for asset in ["assets/models/rei(mu).glb", "assets/models/bisket.glb"] {
        let mut f = Fixture::new(Some(asset));
        for drop in [0.0, 0.2, 0.4, 0.55, 0.2, 0.0] {
            f.drop_body(drop);
            f.assert_pose();
            assert_eq!(
                f.systems.lower_body_ik.status(f.avatar),
                LowerBodyStatus::Ready,
                "{asset} at {drop}"
            );
            for foot in f.feet() {
                assert!(
                    (foot.solved_sole[1] - f.base_height).abs() < 0.0001,
                    "{asset}: {foot:?}"
                );
                assert!(foot.residual < 0.0001);
            }
        }
        f.drop_body(0.85);
        f.assert_pose();
        assert_eq!(
            f.systems.lower_body_ik.status(f.avatar),
            LowerBodyStatus::Limited
        );
        for foot in f.feet() {
            assert!((foot.base_sole[0] - foot.solved_sole[0]).abs() < 0.0001);
            assert!((foot.base_sole[2] - foot.solved_sole[2]).abs() < 0.0001);
            assert!(foot.residual > 0.1);
        }
    }
}

#[test]
fn crouch_depth_limits_report_residual_instead_of_moving_tracking() {
    let mut f = Fixture::new(None);
    let tracked = matrix(&f.world, f.driven).unwrap();
    f.drop_body(0.85);
    f.assert_pose();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Limited
    );
    for foot in f.feet() {
        assert!(foot.residual > 0.1);
        assert!(foot.knee_flexion <= 150_f32.to_radians() + 0.0001);
    }
    assert_eq!(matrix(&f.world, f.driven).unwrap(), tracked);
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .crouch_knee_limit = 0.0;
    f.step();
    f.assert_pose(); // Straight-only is a valid, bounded configuration.
}

#[test]
fn crouch_support_is_per_foot_bounded_and_updates_when_floor_changes() {
    let mut f = Fixture::new(None);
    f.world
        .get_component_by_id_as_mut::<ZoneComponent>(f.floor_zone)
        .unwrap()
        .shape = crate::engine::ecs::component::CollisionShape::cube_half_extents([0.1, 0.05, 3.0]);
    let floor = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.floor)
        .unwrap()
        .transform;
    floor.translation[0] = -0.12;
    floor.recompute_model();
    f.drop_body(0.3);
    f.assert_pose();
    assert_eq!(f.feet()[0].support, Some(f.floor_zone));
    assert_eq!(f.feet()[1].support, None);
    assert!(f.feet()[0].solved_sole[1].abs() < 0.0001);
    assert!((f.feet()[1].solved_sole[1] + 0.3).abs() < 0.0001);
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = false;
    f.step();
    f.assert_pose();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NoSupport
    );
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = true;
    f.step();
    f.assert_pose();
    assert_eq!(f.feet()[0].support, Some(f.floor_zone));
    // A floor under the capsule is no longer support once the proxy is airborne.
    let t = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.capsule)
        .unwrap()
        .transform;
    t.translation[1] += 0.5;
    t.recompute_model();
    f.propagate();
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NoSupport
    );
    let t = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.capsule)
        .unwrap()
        .transform;
    t.translation[1] -= 0.5;
    t.recompute_model();
    f.propagate();
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Ready
    );
    f.world.remove_component_subtree(f.floor_collider).unwrap();
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NoSupport
    );
}

#[test]
fn crouch_gates_tracking_opt_out_calibration_and_invalid_scale_then_recovers() {
    let mut f = Fixture::new(None);
    f.drop_body(0.3);
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .crouch_enabled = false;
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Disabled
    );
    close(
        point(matrix(&f.world, f.joints[0][2]).unwrap(), f.offsets[0]),
        [-0.12, -0.3, 0.0],
    );
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .crouch_enabled = true;
    f.world
        .get_component_by_id_as_mut::<InputXRComponent>(f.input)
        .unwrap()
        .pose_valid = false;
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Pending
    );
    f.world
        .get_component_by_id_as_mut::<InputXRComponent>(f.input)
        .unwrap()
        .pose_valid = true;
    f.step();
    f.assert_pose();
    let offsets = f
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .crouch_sole_offsets
        .take();
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NeedsSoleCalibration
    );
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .crouch_sole_offsets = offsets;
    let root = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.root)
        .unwrap()
        .transform;
    root.scale = [1.0, 1.2, 1.0];
    root.recompute_model();
    f.propagate();
    f.step();
    assert!(matches!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::UnsupportedRig(_)
    ));
    let root = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.root)
        .unwrap()
        .transform;
    root.scale = [1.0; 3];
    root.recompute_model();
    f.propagate();
    f.step();
    f.assert_pose();
    let foot = f.joints[0][2];
    f.world.set_parent(foot, Some(f.scene)).unwrap();
    let rotation = quat_rotation_y(0.6);
    let t = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(foot)
        .unwrap()
        .transform;
    t.rotation = rotation;
    t.recompute_model();
    f.propagate();
    f.step();
    assert!(matches!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::UnsupportedRig(_)
    ));
    assert_eq!(
        f.world
            .get_component_by_id_as::<TransformComponent>(foot)
            .unwrap()
            .transform
            .rotation,
        rotation,
        "detached joint belongs to its new parent"
    );
    f.world.set_parent(foot, Some(f.joints[0][1])).unwrap();
    f.propagate();
    f.step();
    f.assert_pose();
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .humanoid_map_generation += 1;
    f.step();
    assert!(matches!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::UnsupportedRig(_)
    ));
    f.systems.lower_body_ik.remove(f.avatar);
    assert!(f.systems.lower_body_ik.feet(f.avatar).is_none());
}

#[test]
fn crouch_rejects_nearby_tables_and_self_colliders_and_respects_yawed_bounds() {
    let mut f = Fixture::new(None);
    for (parent, t, extents) in [
        (f.scene, [0.0, 0.3, 0.0], [0.5, 0.1, 0.5]),
        (f.driven, [0.0, -1.65, 0.0], [3.0, 0.05, 3.0]),
    ] {
        let frame = f
            .world
            .add_component(TransformComponent::new().with_position(t[0], t[1], t[2]));
        f.world.add_child(parent, frame).unwrap();
        let zone = f.world.add_component(ZoneComponent::cube(extents));
        f.world.add_child(frame, zone).unwrap();
        let c = f.world.add_component(CollidableComponent::static_());
        f.world.add_child(zone, c).unwrap();
    }
    f.drop_body(0.3);
    for foot in f.feet() {
        assert_eq!(foot.support, Some(f.floor_zone));
    }
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = false;
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NoSupport
    );
    // Thin yawed surface's AABB overlaps a sole; its actual footprint doesn't.
    f.world
        .get_component_by_id_as_mut::<CollidableComponent>(f.floor_collider)
        .unwrap()
        .enabled = true;
    f.world
        .get_component_by_id_as_mut::<ZoneComponent>(f.floor_zone)
        .unwrap()
        .shape =
        crate::engine::ecs::component::CollisionShape::cube_half_extents([0.5, 0.05, 0.01]);
    let t = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.floor)
        .unwrap()
        .transform;
    t.rotation = quat_rotation_y(0.8);
    t.recompute_model();
    f.propagate();
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::NoSupport
    );
}

#[test]
fn crouch_uniform_scale_and_body_yaw_preserve_geometry() {
    let mut f = Fixture::new(Some("assets/models/rei(mu).glb"));
    let root = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.root)
        .unwrap()
        .transform;
    root.scale = [1.7; 3];
    root.rotation = quat_rotation_y(0.9);
    root.translation = [0.3, -1.6 - 0.5, 0.4];
    root.recompute_model();
    let ground = f.base_height * 1.7;
    let floor = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.floor)
        .unwrap()
        .transform;
    floor.translation[1] = ground - 0.05;
    floor.recompute_model();
    let capsule = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.capsule)
        .unwrap()
        .transform;
    capsule.translation = [0.3, 1.0 + ground, 0.4];
    capsule.recompute_model();
    f.propagate();
    f.step();
    f.assert_pose();
    for foot in f.feet() {
        assert!((foot.solved_sole[1] - ground).abs() < 0.0001);
        assert!(foot.residual < 0.0001);
    }
}

#[test]
fn crouch_mms_configuration_and_validation() {
    use crate::scripting::runner::RuntimeSpecSession;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let (_session,_)=RuntimeSpecSession::start("AVC\n    .crouch_enabled(false)\n    .crouch_sole_offsets([0.0,-0.1,0.0],[0.0,-0.2,0.0])\n    .crouch_knee_pole([0.0,0.0,1.0])\n    .crouch_knee_limit(2.4)\n    .crouch_support_margin(0.04)",&mut world,&mut rx,None,&mut queue).unwrap();
    let c = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<AvatarControlComponent>(id))
        .unwrap();
    assert!(!c.crouch_enabled);
    assert_eq!(c.crouch_sole_offsets.unwrap()[1], [0.0, -0.2, 0.0]);
    assert_eq!(c.crouch_knee_limit, 2.4);
    assert_eq!(c.crouch_support_margin, 0.04);
    assert!(
        AvatarControlComponent::new()
            .with_crouch_sole_offsets([f32::NAN; 3], [0.0; 3])
            .is_err()
    );
    assert!(
        AvatarControlComponent::new()
            .with_crouch_knee_pole([0.0; 3])
            .is_err()
    );
    assert!(
        AvatarControlComponent::new()
            .with_crouch_knee_pole([f32::MAX; 3])
            .is_err()
    );
    assert!(
        AvatarControlComponent::new()
            .with_crouch_knee_limit(std::f32::consts::PI)
            .is_err()
    );
    assert!(
        AvatarControlComponent::new()
            .with_crouch_support_margin(-0.1)
            .is_err()
    );
}

#[test]
fn crouch_full_avatar_stage_initializes_and_propagates_current_leg_pose() {
    let mut f = Fixture::new(Some("assets/models/rei(mu).glb"));
    f.systems.register_avatar_control(f.avatar);
    f.systems
        .lower_body_ik
        .avatars
        .get_mut(&f.avatar)
        .unwrap()
        .rig = None;
    let driven = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.driven)
        .unwrap()
        .transform;
    driven.translation[1] = 1.15;
    driven.recompute_model();
    f.propagate();
    let tracked = matrix(&f.world, f.driven).unwrap();
    let capsule = matrix(&f.world, f.capsule).unwrap();
    f.systems.tick(
        &mut f.world,
        &mut f.visuals,
        &mut f.assets,
        &Default::default(),
        &mut f.queue,
        1.0 / 60.0,
    );
    assert!(
        f.world
            .get_component_by_id_as::<AvatarControlComponent>(f.avatar)
            .unwrap()
            .head_mount
            .is_some()
    );
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Ready
    );
    f.assert_pose();
    for foot in f.feet() {
        assert!(foot.penetration > 0.2);
        assert!(foot.residual < 0.001);
    }
    assert_eq!(matrix(&f.world, f.driven).unwrap(), tracked);
    assert_eq!(matrix(&f.world, f.capsule).unwrap(), capsule);
    let first = f.feet();
    f.systems.tick(
        &mut f.world,
        &mut f.visuals,
        &mut f.assets,
        &Default::default(),
        &mut f.queue,
        1.0 / 60.0,
    );
    f.assert_pose();
    for (a, b) in f.feet().into_iter().zip(first) {
        close(a.solved_sole, b.solved_sole);
    }
}

#[test]
fn crouch_mount_suspends_and_dismount_rechecks_support() {
    use crate::engine::ecs::component::{
        ComponentRef, InputXRGamepadComponent, MountableComponent, PointerComponent, RiderComponent,
    };
    let mut f = Fixture::new(None);
    f.drop_body(0.3);
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .walking_enabled = true;
    f.step();
    assert!(f.feet().iter().all(|f| f.phase == FootPhase::Planted));
    let reference =
        |world: &World, id| ComponentRef::Guid(world.get_component_record(id).unwrap().guid);
    let gamepad = f.world.add_component(InputXRGamepadComponent::new());
    f.world.add_child(f.input, gamepad).unwrap();
    let pointer = f.world.add_component(PointerComponent::new());
    f.world.add_child(f.driven, pointer).unwrap();
    let rider = f.world.add_component(
        RiderComponent::new()
            .anchor(reference(&f.world, f.driven))
            .movement_root(reference(&f.world, f.scene))
            .input(reference(&f.world, gamepad)),
    );
    f.world.add_child(f.driven, rider).unwrap();
    let car = f.world.add_component(TransformComponent::new());
    let entry = f.world.add_component(ZoneComponent::sphere(3.0));
    f.world.add_child(car, entry).unwrap();
    let mount = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, 1.6, 0.0));
    f.world.add_child(car, mount).unwrap();
    let exit = f
        .world
        .add_component(TransformComponent::new().with_position(0.0, 1.6, 0.0));
    f.world.add_child(car, exit).unwrap();
    let mountable = f.world.add_component(
        MountableComponent::new()
            .entry_zone(reference(&f.world, entry))
            .mount_anchor(reference(&f.world, mount))
            .dismount_anchor(reference(&f.world, exit)),
    );
    f.world.add_child(car, mountable).unwrap();
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, car);
    assert!(
        f.systems
            .attachment
            .try_mount_from_hit(&mut f.world, pointer, car, &mut f.queue)
    );
    f.step();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Mounted
    );
    assert!(f.systems.lower_body_ik.feet(f.avatar).is_none());
    assert!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .swing_side
            .is_none()
    );
    f.systems.attachment.begin_frame();
    assert!(
        f.systems
            .attachment
            .try_dismount_for_pointer(&mut f.world, pointer, &mut f.queue)
    );
    f.propagate();
    f.step();
    f.assert_pose();
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Ready
    );
}

#[test]
fn crouch_two_avatars_share_the_index_and_keep_independent_poses() {
    let mut f = Fixture::new(None);
    f.drop_body(0.4);
    let prototype = f.systems.lower_body_ik.avatars[&f.avatar]
        .rig
        .as_ref()
        .unwrap()
        .clone();
    let mut xr = InputXRComponent::on();
    xr.pose_valid = true;
    let input = f.world.add_component(xr);
    f.world.add_child(f.scene, input).unwrap();
    let driver = f.world.add_component(TransformComponent::new());
    f.world.add_child(input, driver).unwrap();
    let avatar = f.world.add_component(AvatarControlComponent::new());
    f.world.add_child(driver, avatar).unwrap();
    let root = f
        .world
        .add_component(TransformComponent::new().with_position(1.0, -0.15, 0.0));
    f.world.add_child(avatar, root).unwrap();
    let hips = named(
        &mut f.world,
        root,
        "other pelvis",
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0; 3],
    );
    let mut slots = BTreeMap::new();
    let mut add = |slot, component| {
        slots.insert(
            slot,
            HumanoidSlotReport {
                slot,
                status: HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                    component,
                    kind: ResolvedHumanoidTargetKind::SkinJoint,
                }),
                provenance: HumanoidSlotProvenance::Explicit,
                diagnostic: None,
            },
        );
    };
    add(HumanoidSlot::Hips, hips);
    for (leg, slots) in prototype.legs.iter().zip([
        [
            HumanoidSlot::LeftUpperLeg,
            HumanoidSlot::LeftLowerLeg,
            HumanoidSlot::LeftFoot,
        ],
        [
            HumanoidSlot::RightUpperLeg,
            HumanoidSlot::RightLowerLeg,
            HumanoidSlot::RightFoot,
        ],
    ]) {
        let mut parent = hips;
        for (rest, slot) in leg.rest.into_iter().zip(slots) {
            let joint = named(
                &mut f.world,
                parent,
                "other leg",
                rest.translation,
                rest.rotation_quat_xyzw,
                rest.scale,
            );
            add(slot, joint);
            parent = joint;
        }
    }
    let report = HumanoidBoneMapReport {
        owning_gltf: root,
        source: None,
        generation: 1,
        valid: true,
        diagnostics: Vec::new(),
        slots,
    };
    let binding = LowerBodyRig::bind(&f.world, root, &report).unwrap();
    let capsule = f
        .world
        .add_component(TransformComponent::new().with_position(1.0, 1.0, 0.0));
    f.world.add_child(f.scene, capsule).unwrap();
    let zone = f.world.add_component(ZoneComponent::capsule_y(0.2, 0.8));
    f.world.add_child(capsule, zone).unwrap();
    let collider = f.world.add_component(CollidableComponent::slide());
    f.world.add_child(zone, collider).unwrap();
    let config = f
        .world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
        .unwrap();
    config.model_root_id = Some(root);
    config.humanoid_map_generation = 1;
    config.crouch_sole_offsets = Some([[0.0, -0.1, 0.0]; 2]);
    config.walking_enabled = false;
    config.capsule_transform_id = Some(capsule);
    config.capsule_collidable_id = Some(collider);
    config.capsule_ready = true;
    f.systems.lower_body_ik.register(avatar);
    f.systems
        .lower_body_ik
        .avatars
        .get_mut(&avatar)
        .unwrap()
        .rig = Some(binding);
    f.propagate();
    f.step();
    f.assert_pose();
    let first = f.feet();
    let second = *f.systems.lower_body_ik.feet(avatar).unwrap();
    for (a, b) in first.into_iter().zip(second) {
        assert!((a.penetration - 0.4).abs() < 0.0001);
        assert!((b.penetration - 0.15).abs() < 0.0001);
        assert!((b.solved_sole[0] - a.solved_sole[0] - 1.0).abs() < 0.0001);
        assert!(b.residual < 0.0001);
    }
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
        .unwrap()
        .crouch_enabled = false;
    f.step();
    for (a, b) in f.feet().into_iter().zip(first) {
        close(a.solved_sole, b.solved_sole);
    }
    // Both gait instances live in this same system. Moving one avatar must not
    // advance the other's swing state or move its retained contacts.
    for id in [f.avatar, avatar] {
        let config = f
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(id)
            .unwrap();
        config.crouch_enabled = true;
        config.walking_enabled = true;
    }
    f.step();
    let second = *f.systems.lower_body_ik.feet(avatar).unwrap();
    for id in [f.root, f.capsule] {
        let t = &mut f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(id)
            .unwrap()
            .transform;
        t.translation[2] += 0.12;
        t.recompute_model();
    }
    f.propagate();
    f.step();
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(f.avatar)
            .unwrap()
            .steps_started
            .iter()
            .sum::<u64>(),
        1
    );
    assert_eq!(
        f.systems
            .lower_body_ik
            .walking(avatar)
            .unwrap()
            .steps_started,
        [0, 0]
    );
    for (a, b) in f
        .systems
        .lower_body_ik
        .feet(avatar)
        .unwrap()
        .iter()
        .zip(second)
    {
        close(a.solved_sole, b.solved_sole);
    }
}

#[test]
fn crouch_map_edits_release_old_joints_and_rebind_without_accumulating_pose() {
    use crate::engine::ecs::component::HumanoidBoneMapComponent;
    let mut f = Fixture::new(Some("assets/models/rei(mu).glb"));
    f.systems.register_avatar_control(f.avatar);
    let driven = &mut f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.driven)
        .unwrap()
        .transform;
    driven.translation[1] = 1.15;
    driven.recompute_model();
    f.propagate();
    f.systems.tick(
        &mut f.world,
        &mut f.visuals,
        &mut f.assets,
        &Default::default(),
        &mut f.queue,
        1.0 / 60.0,
    );
    f.assert_pose();
    let first = f.feet();
    let avc = f
        .world
        .get_component_by_id_as::<AvatarControlComponent>(f.avatar)
        .unwrap();
    let gltf = avc.humanoid_map_gltf.unwrap();
    let old_binding = f.systems.lower_body_ik.avatars[&f.avatar]
        .rig
        .as_ref()
        .unwrap()
        .clone();
    let map = f
        .world
        .add_component(HumanoidBoneMapComponent::new().with_absent(HumanoidSlot::LeftFoot));
    f.world.add_child(gltf, map).unwrap();
    f.systems
        .humanoid_bone_map
        .register_component(&mut f.world, map);
    f.systems.tick(
        &mut f.world,
        &mut f.visuals,
        &mut f.assets,
        &Default::default(),
        &mut f.queue,
        1.0 / 60.0,
    );
    assert!(matches!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::UnsupportedRig(_)
    ));
    for leg in old_binding.legs {
        for (id, rest) in leg.joints.into_iter().zip(leg.rest) {
            assert_eq!(
                f.world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .unwrap()
                    .transform
                    .rotation,
                rest.rotation_quat_xyzw
            );
        }
    }
    f.systems
        .humanoid_bone_map
        .unregister_component(&mut f.world, map);
    f.world.remove_component_subtree(map).unwrap();
    f.systems.tick(
        &mut f.world,
        &mut f.visuals,
        &mut f.assets,
        &Default::default(),
        &mut f.queue,
        1.0 / 60.0,
    );
    assert_eq!(
        f.systems.lower_body_ik.status(f.avatar),
        LowerBodyStatus::Ready
    );
    f.assert_pose();
    for (a, b) in f.feet().into_iter().zip(first) {
        close(a.solved_sole, b.solved_sole);
    }
}

#[test]
fn leg_world_target_survives_body_translation_and_yaw_on_actual_rigs() {
    use super::leg::solve_leg;
    use super::rig::{emit_pose, uniform};
    use crate::utils::math::{mat4_mul, quat_rotate_vec3};
    for asset in [
        None,
        Some("assets/models/rei(mu).glb"),
        Some("assets/models/bisket.glb"),
    ] {
        let mut f = Fixture::new(asset);
        f.drop_body(0.2);
        let goal = FootGoal {
            sole: f.feet()[0].solved_sole,
            rotation: uniform(matrix(&f.world, f.joints[0][2]).unwrap())
                .unwrap()
                .rotation_quat_xyzw,
        };
        let root = &mut f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(f.root)
            .unwrap()
            .transform;
        root.translation[0] += 0.06;
        root.translation[2] += 0.10;
        root.rotation = quat_rotation_y(0.35);
        root.recompute_model();
        f.propagate();
        let rig = f.systems.lower_body_ik.avatars[&f.avatar]
            .rig
            .as_ref()
            .unwrap();
        let leg = &rig.legs[0];
        let upper = mat4_mul(
            matrix(&f.world, leg.parent).unwrap(),
            leg.rest[0].to_matrix().unwrap(),
        );
        let pole = quat_rotate_vec3(quat_rotation_y(0.35), [0.0, 0.0, 1.0]);
        let solution =
            solve_leg(leg, upper, f.offsets[0], goal, pole, 150_f32.to_radians()).unwrap();
        close(solution.sole, goal.sole);
        for (id, local) in leg.joints.into_iter().zip(solution.local) {
            emit_pose(&mut f.queue, id, local);
        }
        f.queue
            .flush(&mut f.world, &mut f.systems, &mut f.visuals, &mut f.assets);
        close(
            point(matrix(&f.world, f.joints[0][2]).unwrap(), f.offsets[0]),
            goal.sole,
        );
        let actual = uniform(matrix(&f.world, f.joints[0][2]).unwrap())
            .unwrap()
            .rotation_quat_xyzw;
        let dot = actual
            .iter()
            .zip(goal.rotation)
            .map(|(a, b)| a * b)
            .sum::<f32>()
            .abs();
        assert!(
            dot > 0.99999,
            "planted orientation rotated with body: {asset:?}"
        );
        for j in 0..3 {
            let actual = &f
                .world
                .get_component_by_id_as::<TransformComponent>(f.joints[0][j])
                .unwrap()
                .transform;
            let rest = f.systems.lower_body_ik.avatars[&f.avatar]
                .rig
                .as_ref()
                .unwrap()
                .legs[0]
                .rest[j];
            close(actual.translation, rest.translation);
            close(actual.scale, rest.scale);
        }
    }
}
