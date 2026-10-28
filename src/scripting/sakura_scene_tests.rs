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
                .filter(|model| model.uri == "assets/models/lumae.glb")
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
    let (_session, _) = super::tests::start_settings_scene(
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
fn sakura_pistol_grab_gates_burst_and_release_only_stops_weapon_flash() {
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
    let (mut session, _) = super::tests::start_settings_scene(
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
    let mode = named(&world, "sakura_pistol_laser_mode_burst");
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        mode,
        EventSignal::Click {
            raycaster: pointer,
            renderable: mode,
            hit_point: [0.; 3],
            screen_pos_px: None,
        },
    );
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
    let flash = named(&world, "sakura_pistol_laser_muzzle_flash");
    assert_eq!(
        world.parent_of(flash),
        Some(named(&world, "sakura_pistol_laser_root"))
    );
    for name in ["first", "second", "third"] {
        assert_eq!(
            world.parent_of(named(&world, &format!("sakura_pistol_laser_{name}_flight"))),
            None
        );
    }
    let can = named(&world, "sakura_bench_can_1_body");
    let point = [-19.2, 26.7, -4.235];
    let hit = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        can,
        EventSignal::RayIntersected {
            raycaster: casts[0],
            renderable: can,
            t: 1.,
            origin: [point[0], point[1], point[2] + 1.],
            dir: [0., 0., -1.],
        },
    );
    let reaction = named(&world, "sakura_bench_can_1_hit_reaction");
    assert!(hit.iter().any(|intent|matches!(intent,IntentValue::SetAnimationState{component_id,state:AnimationState::Playing} if *component_id==reaction)));
    let number = named(&world, "hit_number");
    assert_eq!(
        world.parent_of(number),
        None,
        "hit feedback must float independently of the can"
    );
    assert_eq!(authored_position(&world, number), point);
    assert!(
        !world.all_components().any(|id| world
            .component_label(id)
            .is_some_and(|name| name.ends_with("_hit_count"))),
        "cans must not have permanent text labels"
    );
    let readout = named(&world, "sakura_can_hit_readout");
    assert!(hit.iter().any(|intent| matches!(intent, IntentValue::SetText {component_id,text} if *component_id == readout && text == "Can hits: 1")));
    let other = named(&world, "sakura_bench_can_2_hit_reaction");
    assert!(!hit.iter().any(|intent| matches!(intent, IntentValue::SetAnimationState {component_id,..} if *component_id == other)));
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        EventSignal::XrButtonUp {
            source_component: pointer,
            hand: ControllerHand::Right,
            control: XrButtonControl::RightTrigger,
            value: 0.,
        },
    );
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
    let flash_clip = named(&world, "sakura_pistol_laser_muzzle_flash_clip");
    assert!(released.iter().any(|intent| matches!(intent, IntentValue::SetAnimationState {component_id,state:AnimationState::Paused} if *component_id==flash_clip)));
    assert!(!released.iter().any(|intent| matches!(intent, IntentValue::SetAnimationState {component_id,..} if shots.contains(component_id))), "release must not cancel a launched projectile");
    assert!(!released.iter().any(|intent| matches!(intent, IntentValue::UpdateTransform {component_id,..} if world.component_label(*component_id).is_some_and(|name| name.ends_with("_beam")))), "release must not hide a launched beam");
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
fn hit_number_rises_and_retires_its_world_subtree() {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{AnimationState, TextComponent, TransitionComponent};
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, initial) = super::tests::start_settings_scene(
        r#"import { hit_number } from "../assets/components/hit-number.mms"
        let number = hit_number({hit_point=[2.0,3.0,4.0] origin=[2.0,3.0,6.0]})
        number.root
        number.animation.play()"#,
        "examples/_hit_number_test.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(initial.errors.is_empty(), "{:?}", initial.errors);
    let root = named(&world, "hit_number");
    let lift = named(&world, "hit_number_lift");
    let text = named(&world, "hit_number_text");
    assert_eq!(world.parent_of(root), None);
    assert_eq!(authored_position(&world, root), [2., 3., 4.]);
    let label = world.get_component_by_id_as::<TextComponent>(text).unwrap();
    assert_eq!(label.text, "1");
    assert!((label.font_size - 0.065).abs() < 1e-6);
    assert!(world.children_of(lift).iter().any(|&id| {
        world
            .get_component_by_id_as::<TransitionComponent>(id)
            .is_some()
    }));
    let clip = named(&world, "hit_number_animation");
    let mut animation = crate::engine::ecs::system::AnimationSystem::new();
    animation.register_animation(&mut world, clip);
    for keyframe in world.children_of(clip).to_vec() {
        animation.register_keyframe(&mut world, keyframe);
    }
    animation.set_animation_state(clip, AnimationState::Playing);
    for beat in [0., 0.6, 1.21] {
        let mut executor =
            crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor {
                session: &mut session,
                render_assets: &mut assets,
                emit: &mut queue,
            };
        animation.tick_with_beat_and_executor(&mut world, beat, 120., &mut rx, Some(&mut executor));
    }
    let intents = rx.drain_ready_intents();
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::UpdateTransform{component_id,translation,..}) if *component_id == lift && translation[1] > 0.4)));
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::RemoveSubtree{component_id}) if *component_id == root)), "transient text must retire its complete subtree");
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
    let (mut session, initial) = super::tests::start_settings_scene(
        r#"import { laser } from "../assets/components/laser.mms"
        import { mini_explosion } from "../assets/components/mini-explosion.mms"
        let source_tool=T {}
        let source_user=T {}
        fn impact_hit(hit) {
            if hit.source_tool == source_tool && hit.source_user == source_user {
                let impact=mini_explosion(hit.hit_point,2.0)
                impact.root
                impact.animation.play()
            }
        }
        let gun = laser({ name = "test_laser" ray_origin_offset = 0.06 })
        T.position(0.0, 0.0, -3.0) {
            R.cube() { name = "laser_target" Raycastable.enabled() }
        }
        gun.launch({pose=source_tool.world_trs() source_tool=source_tool source_user=source_user on_hit=impact_hit})"#,
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
fn sakura_pistol_modes_have_continuous_auto_and_independent_triggers() {
    use crate::engine::ecs::component::{
        AnimationComponent, AnimationState, AudioClipComponent, ControllerHand, KeyframeComponent,
        SelectionComponent, XrButtonControl,
    };
    use crate::engine::ecs::system::AnimationSystem;
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
    let grip = named(&world, "sakura_machine_pistol_placement");
    let right = named(&world, "sakura_right_hand_pointer");
    let left = named(&world, "sakura_left_hand_pointer");
    assert!(
        world
            .get_component_by_id_as::<SelectionComponent>(named(
                &world,
                "sakura_pistol_laser_mode_selection"
            ))
            .is_some()
    );
    let selector = world
        .get_component_by_id_as::<TransformComponent>(named(
            &world,
            "sakura_pistol_laser_mode_selector",
        ))
        .unwrap();
    assert_eq!(selector.transform.scale, [0.0018; 3]);
    let grab = |pointer, target| EventSignal::GrabStart {
        pointer,
        raycaster: pointer,
        renderable: target,
        target,
        ray_origin_world: [0.; 3],
        ray_dir_world: [0., 0., -1.],
    };
    let click = |renderable| EventSignal::Click {
        raycaster: left,
        renderable,
        hit_point: [0.; 3],
        screen_pos_px: None,
    };
    let down = |hand| EventSignal::XrButtonDown {
        source_component: right,
        hand,
        control: if hand == ControllerHand::Right {
            XrButtonControl::RightTrigger
        } else {
            XrButtonControl::LeftTrigger
        },
        value: 1.,
    };
    let up = |hand| EventSignal::XrButtonUp {
        source_component: right,
        hand,
        control: if hand == ControllerHand::Right {
            XrButtonControl::RightTrigger
        } else {
            XrButtonControl::LeftTrigger
        },
        value: 0.,
    };
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        grip,
        grab(right, grip),
    );
    let auto = named(&world, "sakura_pistol_laser_mode_auto");
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        auto,
        click(auto),
    );
    let mut animation = AnimationSystem::new();
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
    let mut sounds = Vec::new();
    let mut shot_times = Vec::new();
    let mut consume = |animation: &mut AnimationSystem, intents: Vec<IntentValue>, step: usize| {
        for intent in intents {
            match intent {
                IntentValue::SetAnimationState {
                    component_id,
                    state,
                } => animation.set_animation_state(component_id, state),
                IntentValue::AudioSchedulePlay { component_id, .. } => {
                    sounds.push((step, component_id))
                }
                IntentValue::RequestRaycast { .. } => shot_times.push(step as f64 * 0.005),
                _ => {}
            }
        }
    };
    let start = deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        ComponentId::default(),
        down(ControllerHand::Right),
    );
    assert!(start.iter().any(|i| matches!(
        i,
        IntentValue::SetAnimationState {
            state: AnimationState::Looping,
            ..
        }
    )));
    consume(&mut animation, start, 0);
    for step in 0..400 {
        // Releasing the other pointer must not interrupt Auto.
        if step == 40 || step == 100 {
            let hand = if step == 40 {
                ControllerHand::Left
            } else {
                ControllerHand::Right
            };
            let intents = deliver_scene_event(
                &mut world,
                &mut rx,
                &mut session,
                &mut assets,
                &mut queue,
                ComponentId::default(),
                up(hand),
            );
            consume(&mut animation, intents, step);
        }
        // Hold Semi for more than its cooldown, then try Burst.
        if step == 140 || step == 260 {
            deliver_scene_event(
                &mut world,
                &mut rx,
                &mut session,
                &mut assets,
                &mut queue,
                ComponentId::default(),
                up(ControllerHand::Right),
            );
            let mode = named(
                &world,
                if step == 140 {
                    "sakura_pistol_laser_mode_semi"
                } else {
                    "sakura_pistol_laser_mode_burst"
                },
            );
            deliver_scene_event(
                &mut world,
                &mut rx,
                &mut session,
                &mut assets,
                &mut queue,
                mode,
                click(mode),
            );
            let intents = deliver_scene_event(
                &mut world,
                &mut rx,
                &mut session,
                &mut assets,
                &mut queue,
                ComponentId::default(),
                down(ControllerHand::Right),
            );
            consume(&mut animation, intents, step);
        }
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
        consume(
            &mut animation,
            rx.drain_ready_intents()
                .into_iter()
                .filter_map(|s| s.intent.map(|i| i.value))
                .collect(),
            step,
        );
        queue.drain_into_rx(&mut rx);
        for event in rx.drain_ready_events() {
            rx.dispatch_event_handlers(&mut world, &event);
        }
        let output = session.service_callbacks(&mut world, &mut rx, Some(&mut assets), &mut queue);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        consume(&mut animation, output.intents, step);
    }
    let automatic: Vec<_> = shot_times.iter().copied().filter(|&t| t < 0.5).collect();
    assert_eq!(
        automatic.len(),
        5,
        "continuous 10 shots/s Auto: {shot_times:?}"
    );
    for pair in automatic.windows(2) {
        assert!(
            (0.095..=0.105001).contains(&(pair[1] - pair[0])),
            "no gap at a cycle boundary: {automatic:?}"
        );
    }
    assert!(
        !shot_times.iter().any(|&t| (0.5..0.7).contains(&t)),
        "release stops future Auto shots"
    );
    assert_eq!(
        shot_times
            .iter()
            .filter(|&&t| (0.7..1.3).contains(&t))
            .count(),
        1,
        "Semi must not repeat while held"
    );
    assert_eq!(
        shot_times.iter().filter(|&&t| t >= 1.3).count(),
        3,
        "Burst fires three shots"
    );
    for (step, id) in sounds {
        assert!(
            !(100..140).contains(&step),
            "release must not queue another automatic recording"
        );
        let clip = world
            .get_component_by_id_as::<AudioClipComponent>(id)
            .unwrap();
        assert_eq!(
            clip.uri,
            if step < 100 {
                "assets/audio/tool/laser_machine_pistol_B.0.wav"
            } else if step < 260 {
                "assets/audio/tool/laser_machine_pistol_A.0.wav"
            } else {
                "assets/audio/tool/laser_machine_pistol_C.0.wav"
            }
        );
    }
    // All three tools own their mode/trigger state, including two-hand use.
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        grip,
        EventSignal::GrabEnd {
            pointer: right,
            target: grip,
        },
    );
    let second = named(&world, "sakura_machine_pistol_2_placement");
    let third = named(&world, "sakura_machine_pistol_3_placement");
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        second,
        grab(right, second),
    );
    deliver_scene_event(
        &mut world,
        &mut rx,
        &mut session,
        &mut assets,
        &mut queue,
        third,
        grab(left, third),
    );
    for (hand, expected) in [
        (ControllerHand::Right, "sakura_pistol_laser_2_semi"),
        (ControllerHand::Left, "sakura_pistol_laser_3_semi"),
    ] {
        let result = deliver_scene_event(
            &mut world,
            &mut rx,
            &mut session,
            &mut assets,
            &mut queue,
            ComponentId::default(),
            down(hand),
        );
        let playing: Vec<_> = result
            .into_iter()
            .filter_map(|intent| match intent {
                IntentValue::SetAnimationState {
                    component_id,
                    state: AnimationState::Playing,
                } => Some(component_id),
                _ => None,
            })
            .collect();
        assert_eq!(playing, [named(&world, expected)]);
    }
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
            .is_some_and(|g| g.uri == "assets/models/lumae.glb")
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
            "assets/models/lumae.glb"
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
        let collisions = named(&world, &format!("{label}_collisions"));
        assert_eq!(world.parent_of(collisions), Some(section));
        assert_eq!(
            world.find_all_components(collisions, "Zone").len(),
            visual_boxes
        );
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
