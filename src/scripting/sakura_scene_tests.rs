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
    let mut shown = std::collections::HashSet::new();
    let mut sounds = Vec::new();
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
                    IntentValue::AudioSchedulePlay { component_id, .. } => {
                        sounds.push(component_id);
                    }
                    IntentValue::SetAnimationState {
                        component_id,
                        state,
                    } => {
                        if state == AnimationState::Playing && shots.contains(&component_id) {
                            fired.push(component_id);
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
    }
    assert_eq!(
        fired, shots,
        "burst must play all three distinct shot clips exactly once, including audio-lookahead scheduling"
    );
    assert_eq!(
        shown.len(),
        3,
        "each child clip must actually display its beam"
    );
    assert_eq!(
        sounds.len(),
        3,
        "one sound per shot, without phase duplication"
    );
    for id in sounds {
        let clip = world
            .get_component_by_id_as::<crate::engine::ecs::component::AudioClipComponent>(id)
            .unwrap();
        assert_eq!(clip.uri, "assets/audio/tool/laser_machine_pistol_C.0.wav");
    }
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
        4
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
}
