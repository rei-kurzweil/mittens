use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::component::material::{MaterialComponent, MaterialDefinitionComponent};
use crate::engine::ecs::component::{
    AvatarControlComponent, BackgroundComponent, Camera3DComponent, CameraXRComponent,
    GLTFComponent, GravityComponent, ZoneComponent,
};
use crate::engine::ecs::{CommandQueue, RxWorld, World};
use crate::engine::graphics::RenderAssets;
use crate::scripting::runner::RuntimeSpecSession;

// Bootstrap materializes local transforms; normal engine initialization later
// propagates cached world matrices. Compose authoring here without starting XR/audio.
fn authored_model(
    world: &World,
    component: ComponentId,
) -> crate::engine::graphics::primitives::TransformMatrix {
    let mut chain = Vec::new();
    let mut node = Some(component);
    while let Some(id) = node {
        if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(id) {
            chain.push(transform.transform.model);
        }
        node = world.parent_of(id);
    }
    chain.into_iter().rev().fold(
        crate::utils::math::mat4_identity(),
        crate::utils::math::mat4_mul,
    )
}
fn authored_position(world: &World, component: ComponentId) -> [f32; 3] {
    let matrix = authored_model(world, component);
    [matrix[3][0], matrix[3][1], matrix[3][2]]
}

#[test]
fn sakura_scene_has_open_ceramic_towers_typed_sky_rooftop_rig_and_reset() {
    let sprite = image::open("assets/textures/cherry-blossoms/sakura-blossom-256.png")
        .unwrap()
        .to_rgba8();
    assert_eq!(sprite.dimensions(), (256, 256));
    assert_eq!(sprite.get_pixel(0, 0)[3], 0);
    assert!(sprite.pixels().any(|pixel| pixel[3] == 255));
    assert!(sprite.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 255));
    for xr in [false, true] {
        let source = include_str!("../../examples/sakura-drone-conversion.mms")
            .replace(
                "let desktop_view_enabled = false",
                &format!("let desktop_view_enabled = {}", !xr),
            )
            .replace(
                "let xr_view_enabled = true",
                &format!("let xr_view_enabled = {xr}"),
            );
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut queue = CommandQueue::new();
        let mut assets = RenderAssets::new();
        let (mut session, _) = super::tests::start_settings_scene(
            &source,
            "examples/sakura-drone-conversion.mms",
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        )
        .unwrap();
        let named = |name| {
            world
                .all_components()
                .find(|&id| world.component_label(id) == Some(name))
                .unwrap()
        };
        let mirror = world
            .all_components()
            .find_map(|id| {
                world.get_component_by_id_as::<crate::engine::ecs::component::MirrorComponent>(id)
            })
            .unwrap();
        assert_eq!(mirror.quality, 1440);
        named("sakura_mirror");
        named("graphics_bloom_toggle");
        named("graphics_bloom_layers_slider");
        let tower_names = [
            "sakura_rooftop_building",
            "sakura_west_tower_-1",
            "sakura_west_tower_1",
            "sakura_east_tower_0",
            "sakura_east_tower_1",
            "sakura_east_tower_2",
        ];
        let towers = world
            .all_components()
            .filter(|&id| {
                world
                    .component_label(id)
                    .is_some_and(|label| tower_names.contains(&label))
            })
            .count();
        assert_eq!(towers, 6);
        assert_eq!(
            world
                .all_components()
                .filter(|&id| world.component_label(id) == Some("sakura_leaf_cluster"))
                .count(),
            3 * 27
        );
        let blossom_cards = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::TextureComponent>(id)
                    .is_some_and(|texture| {
                        texture.uri()
                            == Some("assets/textures/cherry-blossoms/sakura-blossom-256.png")
                    })
            })
            .count();
        assert_eq!(
            blossom_cards, 3,
            "one retained texture per combined blossom mesh"
        );

        let models: Vec<_> = world
            .all_components()
            .filter_map(|id| world.get_component_by_id_as::<GLTFComponent>(id))
            .collect();
        assert_eq!(
            models
                .iter()
                .filter(|model| model.uri == "assets/models/car.glb")
                .count(),
            2
        );
        assert_eq!(
            models
                .iter()
                .filter(|model| model.uri == "assets/models/sakura-machine-pistol.glb")
                .count(),
            3
        );
        assert_eq!(
            models
                .iter()
                .filter(|model| model.uri == "assets/components/characters/lumae/lumae.glb")
                .count(),
            0
        );
        assert!(authored_position(&world, named("sakura_pistol_table"))[0] < -18.0);
        assert!(authored_position(&world, named("sakura_lumae_pedestal"))[2] > -1.4);
        let background = world
            .all_components()
            .filter_map(|id| world.get_component_by_id_as::<BackgroundComponent>(id))
            .collect::<Vec<_>>();
        assert_eq!(background.len(), 2);
        assert_eq!(
            background
                .iter()
                .filter(|bg| bg.occlusion_and_lighting)
                .count(),
            1
        );
        let sky_definition = world
            .all_components()
            .find_map(|id| {
                world
                    .get_component_by_id_as::<MaterialDefinitionComponent>(id)
                    .filter(|definition| {
                        definition.fragment_uri == "assets/shaders/sky-gradient.frag"
                    })
            })
            .unwrap();
        let compiled = crate::engine::graphics::custom_material::compile(
            &sky_definition.fragment_uri,
            &sky_definition.fields,
        )
        .unwrap();
        assert_eq!(compiled.offsets.len(), 8);
        assert_eq!(compiled.byte_size, 32);
        let tile_definition = world
            .all_components()
            .find_map(|id| {
                world
                    .get_component_by_id_as::<MaterialDefinitionComponent>(id)
                    .filter(|definition| {
                        definition.fragment_uri == "assets/shaders/pool-tiles.frag"
                    })
            })
            .unwrap();
        assert_eq!(tile_definition.fields.len(), 7);
        assert!(world.all_components().any(|id| {
            world
                .get_component_by_id_as::<MaterialComponent>(id)
                .is_some()
        }));
        let grip = named("sakura_machine_pistol_placement");
        let marker = world
            .children_of(grip)
            .iter()
            .find_map(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::GrabbableComponent>(id)
            })
            .unwrap();
        assert!(marker.enabled && marker.to_hand && !marker.move_parent);
        assert_eq!(marker.hand_offset, [0., 0., -0.105]);
        let tracked_hands: Vec<_> = world
            .all_components()
            .filter_map(|id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::HandTrackingComponent>(
                        id,
                    )
            })
            .collect();
        assert_eq!(tracked_hands.len(), usize::from(xr));
        if xr {
            assert_eq!(
                tracked_hands[0].provider,
                Some(crate::engine::ecs::system::hand_tracking::HandTrackingProvider::OpenXr)
            );
        }
        let pistol = named("sakura_machine_pistol");
        let model = authored_model(&world, pistol);
        assert!(
            (model[3][2] - 0.2).abs() < 0.001,
            "pistol should be on the spawn-side weapon table"
        );
        for (column, row) in [(0, 1), (1, 2), (2, 0)] {
            assert!(
                (model[column][row] - 0.085).abs() < 0.001,
                "grip authoring must preserve the original tabletop orientation"
            );
        }
        let reset = named("sakura_roof_reset_sensor");
        let sensor = world
            .get_component_by_id_as::<ZoneComponent>(reset)
            .unwrap();
        assert!(sensor.events_enabled);
        assert_eq!(authored_position(&world, reset), [0., 2., 0.]);
        assert_eq!(
            world
                .all_components()
                .filter(|&id| world
                    .get_component_by_id_as::<CameraXRComponent>(id)
                    .is_some())
                .count(),
            usize::from(xr)
        );
        assert_eq!(
            world
                .all_components()
                .filter(|&id| world
                    .get_component_by_id_as::<Camera3DComponent>(id)
                    .is_some())
                .count(),
            usize::from(!xr)
        );
        if xr {
            assert!(
                world
                    .get_component_by_id_as::<AvatarControlComponent>(named(
                        "sakura_mitten_avatar_control"
                    ))
                    .is_some()
            );
            assert!(
                !world
                    .get_component_by_id_as::<GravityComponent>(named("sakura_mitten_gravity"))
                    .unwrap()
                    .enabled
            );
            let position = authored_position(&world, named("sakura_mitten_locomotion_root"));
            assert!((position[1] - 25.63).abs() < 0.001);
        }
        // Check actual authored static collision geometry, not just decorative bays.
        let solid_at = |point: [f32; 3]| {
            world.all_components().any(|id| {
            let Some(zone) = world.get_component_by_id_as::<ZoneComponent>(id) else {return false};
            if !world.children_of(id).iter().any(|&child| world.get_component_by_id_as::<crate::engine::ecs::component::CollidableComponent>(child).is_some()) { return false; }
            let crate::engine::ecs::system::model::collision_types::CollisionShape::Cube {half_extents} = zone.shape else {return false};
            let model = authored_model(&world, id);
            let Some(inverse) = crate::utils::math::mat4_inverse(model) else {return false};
            let p = crate::utils::math::mat4_mul_vec4(inverse,[point[0],point[1],point[2],1.]);
            (0..3).all(|axis| p[axis].abs() <= half_extents[axis] + 0.0001)
        })
        };
        assert!(
            !solid_at([-11., 11.4, 0.]),
            "an upper-storey window must be a real opening"
        );
        assert!(
            solid_at([-11., 10.05, 0.]),
            "ceramic spandrel below the window"
        );
        assert!(
            solid_at([-18., 25.5, 0.]),
            "spawn roof has a real collidable slab"
        );
        assert!(
            !solid_at([-22.6, 25.55, 3.]),
            "stairwell roof opening remains clear"
        );
        if xr {
            let movement_target = named("sakura_mitten_locomotion_root");
            let offset = [0.12, -0.9, 0.0];
            rx.dispatch_event_handlers(
                &mut world,
                &crate::engine::ecs::Signal::event(
                    reset,
                    crate::engine::ecs::EventSignal::ZoneEntered {
                        zone: reset,
                        other_zone: reset,
                        collidable: reset,
                        movement_target,
                        movement_target_offset: offset,
                    },
                ),
            );
            let output =
                session.service_callbacks(&mut world, &mut rx, Some(&mut assets), &mut queue);
            assert!(output.errors.is_empty(), "{:?}", output.errors);
            let translation = output
                .intents
                .iter()
                .find_map(|intent| match intent {
                    crate::engine::ecs::IntentValue::UpdateTransform {
                        component_id,
                        translation,
                        ..
                    } if *component_id == movement_target => Some(*translation),
                    _ => None,
                })
                .expect("reset callback must request moving the locomotion root");
            assert!((translation[0] - -17.88).abs() < 0.001);
            assert!(
                (translation[1] - 25.9).abs() < 0.001,
                "reset must account for the capsule/locomotion offset: {translation:?}"
            );
            assert!((translation[2] - -1.4).abs() < 0.001);
            assert!(
                output.intents.iter().any(|intent| matches!(
                    intent,
                    crate::engine::ecs::IntentValue::VelocityReset { .. }
                )),
                "fall velocity must be reset"
            );
        }
    }
}

#[test]
fn mitten_garden_uses_the_same_sakura_leaf_prefab() {
    let source = include_str!("../../examples/mitten.mms");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (_session, _) = super::tests::start_settings_scene(
        source,
        "examples/mitten.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world.component_label(id) == Some("sakura_leaf_cluster"))
            .count(),
        27
    );
    assert!(
        world
            .all_components()
            .any(|id| world.component_label(id) == Some("garden_tree"))
    );
}

fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}

#[test]
fn component_field_example_uses_queries_and_shared_function_values() {
    let source = format!(
        "{}\nText {{ name = \"field_example_result\" description }}\n",
        include_str!("../../examples/proposals/component-instance-methods.mms")
    );
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (_, output) = super::tests::start_settings_scene(
        &source,
        "examples/proposals/component-instance-methods.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let result = world
        .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(named(
            &world,
            "field_example_result",
        ))
        .unwrap();
    assert_eq!(result.text, "Lime can: 0 hits");
}

fn deliver_scene_event(
    world: &mut World,
    rx: &mut RxWorld,
    session: &mut RuntimeSpecSession,
    assets: &mut RenderAssets,
    queue: &mut CommandQueue,
    scope: ComponentId,
    event: crate::engine::ecs::EventSignal,
) -> Vec<crate::engine::ecs::IntentValue> {
    rx.dispatch_event_handlers(world, &crate::engine::ecs::Signal::event(scope, event));
    let result = session.service_callbacks(world, rx, Some(assets), queue);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    result.intents
}

#[test]
fn can_hit_callback_spawns_feedback_through_the_live_world() {
    let mut scene = crate::engine::Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { metal_can } from "../assets/components/targets/metal-can.mms"
        let can = metal_can({name="live_can" material=Material(MaterialDefinition.toon()) position=[0.0,0.0,0.0]})
        can
        let nearby = metal_can({name="nearby_can" material=Material(MaterialDefinition.toon()) position=[0.3,0.0,0.0]})
        nearby
        on_global("DataEvent", fn(event) {
            if event == "Hit" {
                let target = query("[name='live_can_body']")
                let hit = {renderable=target hit_point=[0.0,0.1,0.045] origin=[0.0,0.1,2.0]}
                target.on_hit(hit)
                nearby.on_blast(hit)
            }
        })
    "#, "examples/_live_can_test.mms").unwrap();
    for _ in 0..2 {
        scene.update(0., &Default::default());
    }
    scene.systems.rx.push_event(
        ComponentId::default(),
        crate::engine::ecs::EventSignal::DataEvent {
            name: "Hit".into(),
            payload: None,
        },
    );
    for _ in 0..2 {
        scene.update(0., &Default::default());
    }
    assert!(
        scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some("hit_number_text"))
    );
    use crate::engine::ecs::component::VelocityComponent;
    let velocity = named(&scene.world, "live_can_velocity");
    let state = scene
        .world
        .get_component_by_id_as::<VelocityComponent>(velocity)
        .unwrap();
    assert!(state.linear_local_mps[2] < -2.0);
    assert!(state.angular_local_radps.iter().any(|v| v.abs() > 0.1));
    let near = named(&scene.world, "nearby_can_velocity");
    assert!(
        scene
            .world
            .get_component_by_id_as::<VelocityComponent>(near)
            .unwrap()
            .linear_local_mps[0]
            > 0.0
    );
    for _ in 0..50 {
        scene.update(0.05, &Default::default());
    }
    scene.systems.rx.push_event(
        ComponentId::default(),
        crate::engine::ecs::EventSignal::DataEvent {
            name: "Hit".into(),
            payload: None,
        },
    );
    scene.update(0.0, &Default::default());
    for _ in 0..60 {
        scene.update(0.05, &Default::default());
    }
    assert!(
        scene
            .world
            .get_component_by_id_as::<VelocityComponent>(velocity)
            .unwrap()
            .linear_local_mps
            .iter()
            .any(|v| *v != 0.0),
        "latest hit must restart the respawn timer"
    );
    for _ in 0..42 {
        scene.update(0.05, &Default::default());
    }
    let state = scene
        .world
        .get_component_by_id_as::<VelocityComponent>(velocity)
        .unwrap();
    assert_eq!(state.linear_local_mps, [0.0; 3]);
    assert_eq!(state.angular_local_radps, [0.0; 3]);
    let pose = &scene
        .world
        .get_component_by_id_as::<TransformComponent>(named(&scene.world, "live_can_motion_pose"))
        .unwrap()
        .transform;
    assert_eq!(pose.translation, [0.0, 0.09, 0.0]);
    assert_eq!(pose.rotation, [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn fired_laser_copies_world_trs_and_stays_put_when_barrel_moves() {
    use crate::engine::ecs::{EventSignal, IntentValue};
    use crate::engine::graphics::primitives::Transform;
    use crate::engine::transform::TransformSpace;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, _) = super::tests::start_settings_scene(
        r#"
        import { laser } from "../assets/components/laser.mms"
        let gun=laser({name="frozen_laser" ray_origin_offset=0.06})
        let muzzle=T {name="frozen_laser_root"}
        T.position(2.0,3.0,4.0).rotation(0.0,0.4,0.0) {
            name="moving_barrel"
            muzzle
        }
        on_global("DataEvent", fn(event) { if event == "Fire" {
            gun.launch({pose=muzzle.world_trs() source_tool=muzzle source_user=null on_hit=null})
        } })
    "#,
        "examples/_frozen_laser_test.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    for id in world.all_components().collect::<Vec<_>>() {
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            let model = authored_model(&world, id);
            world
                .get_component_by_id_as_mut::<TransformComponent>(id)
                .unwrap()
                .transform
                .matrix_world = model;
        }
    }
    let flight = named(&world, "frozen_laser_flight");
    let muzzle = named(&world, "frozen_laser_root");
    let source = named(&world, "moving_barrel");
    assert_eq!(world.parent_of(flight), None);
    let before = crate::engine::ecs::system::TransformSystem::world_trs(&world, muzzle).unwrap();
    let intents = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        EventSignal::DataEvent {
            name: "Fire".into(),
            payload: None,
        },
    );
    let snapshot = intents
        .iter()
        .find_map(|intent| match intent {
            IntentValue::SetTransformTrs {
                component_id,
                space: TransformSpace::World,
                value,
            } if *component_id == flight => Some(*value),
            _ => None,
        })
        .expect("a shot must copy the barrel's world TRS into a separate world root");
    assert_eq!(snapshot, before);
    world
        .get_component_by_id_as_mut::<TransformComponent>(flight)
        .unwrap()
        .transform = Transform::try_from(snapshot).unwrap();
    let frozen = authored_model(&world, flight);
    *world
        .get_component_by_id_as_mut::<TransformComponent>(source)
        .unwrap() = TransformComponent::new().with_position(20., 30., 40.);
    assert_ne!(authored_model(&world, muzzle), frozen);
    assert_eq!(
        authored_model(&world, flight),
        frozen,
        "moving the gun must not drag an existing beam"
    );
}

#[test]
fn sakura_pedestal_spawns_lumae_with_pose_and_hair_in_clear_rooftop_positions() {
    use crate::engine::ecs::{EventSignal, IntentValue};
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, _) = super::tests::start_settings_scene(
        include_str!("../../examples/sakura-drone-conversion.mms"),
        "examples/sakura-drone-conversion.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(!world.all_components().any(|id| {
        world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|g| g.uri == "assets/components/characters/lumae/lumae.glb")
    }));
    let button = named(&world, "sakura_add_lumae_button");
    let readout = named(&world, "sakura_lumae_count");
    let mut positions = Vec::new();
    for count in 1..=12 {
        let result = deliver_scene_event(
            &mut world,
            &mut rx,
            &mut session,
            &mut assets,
            &mut queue,
            button,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: button,
                hit_point: [0.; 3],
                screen_pos_px: None,
            },
        );
        assert!(result.iter().any(|intent|matches!(intent, IntentValue::SetText {component_id, text} if *component_id == readout && *text == format!("Lumae: {count}"))));
        let avatar = named(&world, &format!("sakura_lumae_{count}"));
        assert_eq!(
            world
                .get_component_by_id_as::<GLTFComponent>(avatar)
                .unwrap()
                .uri,
            "assets/components/characters/lumae/lumae.glb"
        );
        assert_eq!(
            world
                .find_all_components(avatar, "*")
                .into_iter()
                .filter(|&id| world
                    .get_component_by_id_as::<crate::engine::ecs::component::SpringBoneComponent>(
                        id
                    )
                    .is_some())
                .count(),
            19
        );
        assert_eq!(
            world.find_all_components(avatar, "*").into_iter().filter(|&id|world.get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(id).is_some()).count(),
            1
        );
        // This fixture services MMS directly; apply the structural command that
        // a Universe update normally consumes after the callback returns.
        for intent in result {
            if let IntentValue::Attach { parent, child } = intent {
                world.add_child(parent, child).unwrap();
            }
        }
        let placement = named(&world, &format!("sakura_lumae_{count}_placement"));
        assert_eq!(
            world.parent_of(placement),
            Some(named(&world, "sakura_lumae_group"))
        );
        let [x, y, z] = authored_position(&world, placement);
        assert_eq!(y, 25.6);
        assert!((-24.2..-11.5).contains(&x) && (-4.5..5.0).contains(&z));
        assert!(
            !(x > -21.2 && x < -13.0 && z < -0.4),
            "firing lane: {x},{z}"
        );
        assert!(!(x > -23.8 && x < -21.4 && z > 1.2), "stairwell: {x},{z}");
        positions.push([x, z]);
    }
    assert_ne!(
        positions[0], positions[9],
        "revisiting an anchor should get fresh jitter"
    );
}

#[test]
fn player_building_chunks_confine_geometry_and_rebuild_only_the_edited_floor() {
    use crate::engine::ecs::component::{CombineMeshComponent, RenderableComponent};
    use crate::engine::ecs::system::{CombineMeshSystem, MeshBoundsSystem, RenderableSystem};
    use crate::engine::graphics::{CpuMesh, MeshHandle, MeshUploader, VisualWorld};
    struct Uploader(u32);
    impl MeshUploader for Uploader {
        fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
            self.0 += 1;
            Ok(MeshHandle(1000 + self.0))
        }
    }
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (_, output) = super::tests::start_settings_scene(r#"
        import { pool_building } from "../assets/components/buildings/pool_building.mms"
        import { pool_tile_material } from "../assets/components/materials/pool_tiles.mms"
        let tiles = pool_tile_material()
        pool_building("home", [0.0,0.0,0.0], 8, 1.0, true, tiles, [0.88,0.65,0.73], true, true)
        pool_building("neighbor", [30.0,0.0,0.0], 8, -1.0, false, tiles, [0.64,0.78,0.86], true, false)
    "#, "examples/_floor_chunks_test.mms", &mut world, &mut rx, Some(&mut assets), &mut queue).unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let combines: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<CombineMeshComponent>(id)
                .is_some()
        })
        .collect();
    assert_eq!(combines.len(), 20);
    for floor in 0..=8 {
        let label = if floor == 8 {
            "home_roof".to_owned()
        } else {
            format!("home_floor_{floor}")
        };
        let section = named(&world, &label);
        let floor_y = floor as f32 * 3.2;
        assert!((authored_position(&world, section)[1] - floor_y).abs() < 1e-5);
        let groups: Vec<_> = world
            .children_of(section)
            .iter()
            .copied()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<CombineMeshComponent>(id)
                    .is_some()
            })
            .collect();
        assert_eq!(groups.len(), 2);
        let mut visual_boxes = 0;
        for group in groups {
            assert!(
                world
                    .get_component_by_id_as::<CombineMeshComponent>(group)
                    .unwrap()
                    .keep_transforms
            );
            for source in world.find_all_components(group, "Renderable") {
                assert!(
                    world
                        .get_component_by_id_as::<RenderableComponent>(source)
                        .is_some()
                );
                visual_boxes += 1;
                let model = authored_model(&world, source);
                // Full-height columns must be split too, and the stairs must
                // belong to the top storey rather than extending down from the roof.
                for x in [-0.5, 0.5] {
                    for y in [-0.5, 0.5] {
                        for z in [-0.5, 0.5] {
                            let point = crate::utils::math::mat4_mul_vec4(model, [x, y, z, 1.]);
                            assert!(
                                point[1] >= floor_y - 0.2201,
                                "{label} reaches below its slab: {point:?}"
                            );
                            let ceiling = if floor == 8 {
                                floor_y + 0.6
                            } else {
                                floor_y + 3.2
                            };
                            assert!(
                                point[1] <= ceiling + 1e-4,
                                "{label} reaches into the next storey: {point:?}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(world.find_all_components(section, "Zone").len(), visual_boxes);
    }
    assert!(
        world
            .get_component_by_id_as::<CombineMeshComponent>(named(&world, "neighbor_ceramic_white"))
            .is_some()
    );
    assert!(
        world
            .get_component_by_id_as::<CombineMeshComponent>(named(&world, "neighbor_ceramic_trim"))
            .is_some()
    );
    let propagate = |world: &mut World| {
        for id in world.all_components().collect::<Vec<_>>() {
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_some()
            {
                let matrix = authored_model(world, id);
                world
                    .get_component_by_id_as_mut::<TransformComponent>(id)
                    .unwrap()
                    .transform
                    .matrix_world = matrix;
            }
        }
    };
    propagate(&mut world);
    let mut combine = CombineMeshSystem::default();
    let mut visuals = VisualWorld::default();
    let mut uploader = Uploader(0);
    let mut renderables = RenderableSystem::default();
    let mut bounds = MeshBoundsSystem::default();
    combine.reconcile_and_build(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut renderables,
        &mut bounds,
    );
    let original: Vec<_> = combines
        .iter()
        .map(|&id| (id, visuals.instance_handle_for_component(id).unwrap()))
        .collect();
    let changed = named(&world, "home_floor_3_ceramic_white");
    let source = world.find_all_components(changed, "Renderable")[0];
    let placement = world.parent_of(source).unwrap();
    let transform = &mut world
        .get_component_by_id_as_mut::<TransformComponent>(placement)
        .unwrap()
        .transform;
    transform.scale[0] *= 0.8;
    transform.recompute_model();
    propagate(&mut world);
    let uploads = uploader.0;
    combine.reconcile_and_build(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut renderables,
        &mut bounds,
    );
    assert_eq!(
        uploader.0,
        uploads + 1,
        "editing one floor must upload only that changed mesh"
    );
    for (id, handle) in original {
        if id == changed {
            assert_ne!(visuals.instance_handle_for_component(id), Some(handle));
        } else {
            assert_eq!(
                visuals.instance_handle_for_component(id),
                Some(handle),
                "another floor, tint group, roof or neighbor was rebuilt: {:?}",
                world.component_label(id)
            );
        }
    }
    // Custom shader compilation is asynchronous. Finish it before this short
    // standalone fixture exits, and verify the retained tile material compiles.
    for (_, instance) in visuals.materials.instances() {
        if let Some(custom) = visuals
            .materials
            .definition(instance.definition)
            .and_then(|definition| definition.custom.as_ref())
        {
            for _ in 0..200 {
                if custom.compiled.get().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .expect("tile shader compiler timed out")
                .as_ref()
                .unwrap();
        }
    }
}

#[path = "sakura_animation_tests.rs"]
mod animation_tests;
