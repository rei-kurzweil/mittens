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
        let (mut session, _) = RuntimeSpecSession::start_at_path(
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
        assert_eq!(blossom_cards, 3 * 27 * 3);

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
            1
        );
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
            (model[3][2] + 3.8).abs() < 0.001,
            "pistol should be near the front of the bench"
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
                        "sakura_reimu_avatar_control"
                    ))
                    .is_some()
            );
            assert!(
                !world
                    .get_component_by_id_as::<GravityComponent>(named("sakura_reimu_gravity"))
                    .unwrap()
                    .enabled
            );
            let position = authored_position(&world, named("sakura_reimu_locomotion_root"));
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
            let movement_target = named("sakura_reimu_locomotion_root");
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
fn rei_mu_garden_uses_the_same_sakura_leaf_prefab() {
    let source = include_str!("../../examples/rei(mu).mms");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (_session, _) = RuntimeSpecSession::start_at_path(
        source,
        "examples/rei(mu).mms",
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
fn sakura_pistol_grab_gates_three_shot_subanimations_and_release_cancels_them() {
    use crate::engine::ecs::component::XrButtonControl;
    use crate::engine::ecs::component::{
        AnimationComponent, AnimationState, ControllerHand, KeyframeComponent,
    };
    use crate::engine::ecs::system::AnimationSystem;
    use crate::engine::ecs::{EventSignal, IntentValue};
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, _) = RuntimeSpecSession::start_at_path(
        include_str!("../../examples/sakura-drone-conversion.mms"),
        "examples/sakura-drone-conversion.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    let grip = named(&world, "sakura_machine_pistol_placement");
    let pointer = named(&world, "sakura_right_hand_pointer");
    let burst = named(&world, "sakura_pistol_laser_burst");
    let shots = ["first", "second", "third"]
        .map(|shot| named(&world, &format!("sakura_pistol_laser_{shot}_shot")));
    let trigger = |hand| EventSignal::XrButtonDown {
        source_component: pointer,
        hand,
        control: if hand == ControllerHand::Right {
            XrButtonControl::RightTrigger
        } else {
            XrButtonControl::LeftTrigger
        },
        value: 1.,
    };
    let unheld = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Right),
    );
    assert!(!unheld.iter().any(|intent| matches!(
        intent,
        IntentValue::SetAnimationState {
            state: AnimationState::Playing,
            ..
        }
    )));
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        grip,
        EventSignal::GrabStart {
            pointer,
            raycaster: pointer,
            renderable: grip,
            target: grip,
            ray_origin_world: [0.; 3],
            ray_dir_world: [0., 0., -1.],
        },
    );
    let wrong_hand = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Left),
    );
    assert!(!wrong_hand.iter().any(|intent| matches!(
        intent,
        IntentValue::SetAnimationState {
            state: AnimationState::Playing,
            ..
        }
    )));
    let start = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Right),
    );
    assert!(start.iter().any(|intent| matches!(intent,IntentValue::SetAnimationState {component_id,state:AnimationState::Playing} if *component_id==burst)),"{start:?}");
    let busy = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Right),
    );
    assert!(
        !busy.iter().any(|intent| matches!(
            intent,
            IntentValue::SetAnimationState {
                state: AnimationState::Playing,
                ..
            }
        )),
        "a trigger cannot restart an active burst"
    );
    let mut animation = AnimationSystem::new();
    let components: Vec<_> = world.all_components().collect();
    for id in components {
        if world
            .get_component_by_id_as::<AnimationComponent>(id)
            .is_some()
        {
            animation.register_animation(&mut world, id);
        }
        if world
            .get_component_by_id_as::<KeyframeComponent>(id)
            .is_some()
        {
            animation.register_keyframe(&mut world, id);
        }
    }
    animation.set_animation_state(burst, AnimationState::Playing);
    let mut fired = Vec::new();
    let mut shot_times = Vec::new();
    let mut shown = std::collections::HashSet::new();
    let mut sounds = Vec::new();
    let mut casts = Vec::new();
    for step in 0..100 {
        let mut executor =
            crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor {
                session: &mut session,
                render_assets: &mut assets,
                emit: &mut queue,
            };
        animation.tick_with_beat_and_executor(
            &mut world,
            step as f64 * 0.01,
            120.,
            &mut rx,
            Some(&mut executor),
        );
        for signal in rx.drain_ready_intents() {
            if let Some(intent) = signal.intent.map(|intent| intent.value) {
                match intent {
                    IntentValue::RequestRaycast { component_id } => casts.push(component_id),
                    IntentValue::AudioSchedulePlay { component_id, .. } => {
                        sounds.push(component_id);
                    }
                    IntentValue::SetAnimationState {
                        component_id,
                        state,
                    } => {
                        if state == AnimationState::Playing && shots.contains(&component_id) {
                            fired.push(component_id);
                            shot_times.push(step as f64 * 0.01 / 2.0);
                        }
                        animation.set_animation_state(component_id, state);
                    }
                    IntentValue::UpdateTransform {
                        component_id,
                        scale,
                        ..
                    } if scale == [1.; 3] => {
                        if world
                            .component_label(component_id)
                            .is_some_and(|label| label.ends_with("_beam"))
                        {
                            shown.insert(component_id);
                        }
                    }
                    _ => {}
                }
            }
        }
        queue.drain_into_rx(&mut rx);
        for event in rx.drain_ready_events() {
            rx.dispatch_event_handlers(&mut world, &event);
        }
        let output = session.service_callbacks(&mut world, &mut rx, Some(&mut assets), &mut queue);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        if step == 65 {
            let busy = deliver_scene_event(
                &mut world,
                &mut rx,
                &mut session,
                &mut assets,
                &mut queue,
                ComponentId::default(),
                trigger(ControllerHand::Right),
            );
            assert!(
                !busy.iter().any(|intent| matches!(
                    intent,
                    IntentValue::SetAnimationState {
                        state: AnimationState::Playing,
                        ..
                    }
                )),
                "audio lookahead must not unlock the gun early"
            );
        }
    }
    assert_eq!(
        fired, shots,
        "burst must play all three distinct shot clips exactly once, including audio-lookahead scheduling"
    );
    for pair in shot_times.windows(2) {
        assert!(
            (2.0 / 75.0..=3.0 / 75.0).contains(&(pair[1] - pair[0])),
            "shots must be separated by 2–3 frames at 75 fps: {shot_times:?}"
        );
    }
    assert_eq!(
        casts.len(),
        3,
        "each visual shot must request its own cast, without audio-lookahead duplication"
    );
    assert_eq!(
        casts
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    for id in &casts {
        let ray = world
            .get_component_by_id_as::<crate::engine::ecs::component::RayCastComponent>(*id)
            .unwrap();
        assert!(ray.parent_forward && ray.nearest_hit_only);
        assert_eq!(ray.max_distance, 40.);
    }
    assert_eq!(
        shown.len(),
        3,
        "each child clip must actually display its beam"
    );
    assert_eq!(
        sounds.len(),
        1,
        "one full-burst recording per accepted trigger"
    );
    for id in sounds {
        let clip = world
            .get_component_by_id_as::<crate::engine::ecs::component::AudioClipComponent>(id)
            .unwrap();
        assert_eq!(clip.uri, "assets/audio/tool/laser_machine_pistol_C.0.wav");
    }
    let next = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Right),
    );
    assert!(next.iter().any(|intent| matches!(intent, IntentValue::SetAnimationState {component_id, state: AnimationState::Playing} if *component_id == burst)), "completion must allow the next burst");
    let released = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        grip,
        EventSignal::GrabEnd {
            pointer,
            target: grip,
        },
    );
    assert_eq!(
        released
            .iter()
            .filter(|intent| matches!(
                intent,
                IntentValue::SetAnimationState {
                    state: AnimationState::Paused,
                    ..
                }
            ))
            .count(),
        3
    );
    assert_eq!(
        released
            .iter()
            .filter(
                |intent| matches!(intent,IntentValue::UpdateTransform {scale,..} if *scale==[0.;3])
            )
            .count(),
        12
    );
    let after = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Right),
    );
    assert!(!after.iter().any(|intent| matches!(
        intent,
        IntentValue::SetAnimationState {
            state: AnimationState::Playing,
            ..
        }
    )));
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        grip,
        EventSignal::GrabStart {
            pointer,
            raycaster: pointer,
            renderable: grip,
            target: grip,
            ray_origin_world: [0.; 3],
            ray_dir_world: [0., 0., -1.],
        },
    );
    let regrabbed = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        trigger(ControllerHand::Right),
    );
    assert!(
        !regrabbed.iter().any(|intent| matches!(
            intent,
            IntentValue::SetAnimationState {
                state: AnimationState::Playing,
                ..
            }
        )),
        "release/regrab must not bypass the unfinished second burst"
    );
}

#[test]
fn laser_hit_spawns_world_space_impact_only_for_its_own_ray_and_despawns() {
    use crate::engine::ecs::component::{
        AnimationComponent, AnimationState, KeyframeComponent, RayCastComponent,
    };
    use crate::engine::ecs::system::pointer_system::{PointerActivations, PointerSystem};
    use crate::engine::ecs::system::{BvhSystem, RayCastSystem};
    use crate::engine::ecs::{EventSignal, IntentValue};
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, initial) = RuntimeSpecSession::start_at_path(
        r#"import { laser } from "../assets/components/laser.mms"
        let gun = laser({ name = "test_laser" sound = false ray_origin_offset = 0.06 })
        gun.root
        T.position(0.0, 0.0, -3.0) {
            R.cube() { name = "laser_target" Raycastable.enabled() }
        }
        gun.fire_laser()"#,
        "examples/_laser_hit_test.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    let ray = named(&world, "test_laser_raycaster");
    assert!(initial.errors.is_empty(), "{:?}", initial.errors);
    assert!(initial.intents.iter().any(|intent| matches!(intent, IntentValue::RequestRaycast { component_id } if *component_id == ray)));
    assert!((authored_model(&world, ray)[3][2] + 0.06).abs() < 1e-6);
    let hit = |raycaster| EventSignal::RayIntersected {
        raycaster,
        renderable: ComponentId::default(),
        t: 2.,
        origin: [2., 3., 4.],
        dir: [0., 0., -1.],
    };
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        hit(ComponentId::default()),
    );
    assert!(
        !world
            .all_components()
            .any(|id| world.component_label(id) == Some("mini_explosion"))
    );
    // Follow a real shot request through geometry intersection and the MMS hit
    // handler, including the offset mount used by the actual pistol.
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
    world
        .get_component_by_id_as_mut::<RayCastComponent>(ray)
        .unwrap()
        .cast_requests = 1;
    let target = named(&world, "laser_target");
    let mut raycasts = RayCastSystem::default();
    let mut visuals = crate::engine::graphics::VisualWorld::default();
    raycasts.register_raycast(&mut world, &mut visuals, ray);
    raycasts.notify_renderable_added(&world, target);
    raycasts.tick_with_queue(
        &mut world,
        &mut visuals,
        &crate::engine::user_input::InputState::default(),
        &mut rx,
        &BvhSystem::default(),
        &PointerActivations::default(),
        &PointerSystem::default(),
        0.016,
    );
    let hits = rx.drain_ready_events();
    assert_eq!(hits.len(), 1);
    let event = hits.into_iter().next().unwrap();
    let output = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        event.scope,
        event.event.unwrap(),
    );
    let impact = named(&world, "mini_explosion");
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(impact)
            .unwrap()
            .transform
            .translation,
        [0., 0., -2.5]
    );
    assert_eq!(
        world.parent_of(impact),
        None,
        "impact must not move with the gun"
    );
    let clip = output
        .iter()
        .find_map(|intent| match intent {
            IntentValue::SetAnimationState {
                component_id,
                state: AnimationState::Playing,
            } => Some(*component_id),
            _ => None,
        })
        .unwrap();
    let mut animation = crate::engine::ecs::system::AnimationSystem::new();
    for id in world.all_components().collect::<Vec<_>>() {
        if world
            .get_component_by_id_as::<AnimationComponent>(id)
            .is_some()
        {
            animation.register_animation(&mut world, id);
        }
        if world
            .get_component_by_id_as::<KeyframeComponent>(id)
            .is_some()
        {
            animation.register_keyframe(&mut world, id);
        }
    }
    animation.set_animation_state(clip, AnimationState::Playing);
    for beat in [0., 0.2, 0.4, 0.7] {
        let mut executor =
            crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor {
                session: &mut session,
                render_assets: &mut assets,
                emit: &mut queue,
            };
        animation.tick_with_beat_and_executor(&mut world, beat, 120., &mut rx, Some(&mut executor));
    }
    let intents = rx.drain_ready_intents();
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::SetEmissiveIntensity{intensity,..}) if *intensity==0.5)));
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::RemoveSubtree{component_id}) if *component_id==impact)), "impact clip must retire its whole subtree");
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
    let (mut session, _) = RuntimeSpecSession::start_at_path(
        r#"
        import { laser } from "../assets/components/laser.mms"
        let gun=laser({name="frozen_laser" sound=false ray_origin_offset=0.06})
        T.position(2.0,3.0,4.0).rotation(0.0,0.4,0.0) {
            name="moving_barrel"
            gun.root
        }
        on_global("DataEvent", fn(event) { if event == "Fire" { gun.fire_laser() } })
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
