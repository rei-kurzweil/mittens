use super::*;

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
    let (mut session, _) = crate::scripting::tests::start_settings_scene(
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
                benchmarks: &mut Default::default(),
                graphics_settings: &mut Default::default(),
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
    let (mut session, initial) = crate::scripting::tests::start_settings_scene(
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
                benchmarks: &mut Default::default(),
                graphics_settings: &mut Default::default(),
            };
        animation.tick_with_beat_and_executor(&mut world, beat, 120., &mut rx, Some(&mut executor));
    }
    let intents = rx.drain_ready_intents();
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::UpdateTransform{component_id,translation,..}) if *component_id == lift && translation[1] > 0.4)));
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::RemoveSubtree{component_id}) if *component_id == root)), "transient text must retire its complete subtree");
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
    let (mut session, initial) = crate::scripting::tests::start_settings_scene(
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
                benchmarks: &mut Default::default(),
                graphics_settings: &mut Default::default(),
            };
        animation.tick_with_beat_and_executor(&mut world, beat, 120., &mut rx, Some(&mut executor));
    }
    let intents = rx.drain_ready_intents();
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::SetEmissiveIntensity{intensity,..}) if *intensity==0.5)));
    assert!(intents.iter().any(|signal| matches!(signal.intent.as_ref().map(|v| &v.value), Some(IntentValue::RemoveSubtree{component_id}) if *component_id==impact)), "impact clip must retire its whole subtree");
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
    let (mut session, _) = crate::scripting::tests::start_settings_scene(
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
                benchmarks: &mut Default::default(),
                graphics_settings: &mut Default::default(),
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
