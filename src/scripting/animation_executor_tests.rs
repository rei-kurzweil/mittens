use super::*;

#[test]
fn mittens_corp_evaluates_with_mitten_player_bow_and_car_mount_fixture() {
    use crate::engine::ecs::component::{
        AmplitudeComponent, AudioInputComponent, AvatarControlComponent, CameraXRComponent,
        CollidableComponent, CollidableMode, CollisionShape, ComponentRef, ControllerXRComponent,
        EditorComponent, EditorPanel, EditorUIComponent, GLTFComponent, HumanoidBoneMapComponent,
        InputXRComponent, InputXRGamepadComponent, MountableComponent, PointerComponent,
        PoseCaptureComponent, ReturnToRestWhenStillComponent, RiderComponent,
        SecondaryMotionComponent, ShadingComponent, ShadingModel, SpringBoneComponent,
        SpringColliderComponent, TransformComponent, XREyeTrackingComponent, XrAxisControl,
        XrButtonControl, ZoneComponent,
    };

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, output) = start_settings_scene(
        include_str!("../../examples/mittens-corp.mms"),
        "examples/mittens-corp.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("mittens-corp retained runtime should start");
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let stage = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("mittens_corp_stage"))
        .expect("Mittens Corp stage");
    let mut pending = vec![stage];
    let mut static_zone_count = 0;
    while let Some(id) = pending.pop() {
        if world.get_component_by_id_as::<ZoneComponent>(id).is_some()
            && world.children_of(id).iter().any(|child| {
                world
                    .get_component_by_id_as::<CollidableComponent>(*child)
                    .is_some_and(|collidable| collidable.mode == CollidableMode::Static)
            })
        {
            static_zone_count += 1;
        }
        pending.extend(world.children_of(id).iter().copied());
    }
    assert_eq!(
        static_zone_count, 4,
        "deck, both steps and back wall provide contact surfaces"
    );

    let driver = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("mitten_xr_driver"))
        .expect("mittens-corp should name its mitten XR driver");
    let input_xr = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_some()
        })
        .expect("mittens-corp should author InputXR");
    let locomotion_root =
        crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform(
            &world, input_xr,
        )
        .expect("InputXRGamepad should resolve the outer locomotion transform");
    assert_eq!(
        world.component_label(locomotion_root),
        Some("mitten_locomotion_root")
    );
    let rider = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<RiderComponent>(id))
        .expect("mittens-corp should declare its mitten Rider");
    assert!(rider.anchor.is_some());
    assert!(rider.movement_root.is_some());
    assert!(rider.input.is_some());

    let rider_anchor = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("mitten_rider_cxr_anchor"))
        .expect("mitten should expose a rider-side CXR anchor");
    let mut rider_ancestor = world.parent_of(rider_anchor);
    while rider_ancestor.is_some() && rider_ancestor != Some(driver) {
        rider_ancestor = rider_ancestor.and_then(|id| world.parent_of(id));
    }
    assert_eq!(rider_ancestor, Some(driver));
    assert!(world.children_of(rider_anchor).iter().any(|id| {
        world
            .get_component_by_id_as::<CameraXRComponent>(*id)
            .is_some()
    }));

    let car_mount = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("left_display_car_cxr_mount"))
        .expect("car should expose its old cockpit CXR offset as a mount target");
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(car_mount)
            .unwrap()
            .translation(),
        [0.0, 4.5, -1.0]
    );
    let mountable_id = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<MountableComponent>(id)
                .is_some()
        })
        .expect("the independent car should be Mountable");
    let mountable = world
        .get_component_by_id_as::<MountableComponent>(mountable_id)
        .unwrap();
    assert!(mountable.entry_zone.is_some());
    assert!(mountable.mount_anchor.is_some());
    assert!(mountable.dismount_anchor.is_some());

    let car_zone = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<ZoneComponent>(id)
                .filter(|_| world.component_label(id) == Some("car_entry_zone"))
        })
        .expect("car should expose a detection-only front entry zone");
    assert_eq!(
        car_zone.shape,
        CollisionShape::cube_half_extents([4.6, 1.4, 1.4])
    );
    assert_eq!(car_zone.roles, vec!["vehicle_entry"]);
    let zone_frame = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_entry_zone_frame"))
        .unwrap();
    assert_eq!(
        car_zone.frame_source,
        Some(ComponentRef::Guid(
            world.get_component_record(zone_frame).unwrap().guid
        ))
    );
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(zone_frame)
            .unwrap()
            .translation(),
        [0.0, 1.0, -1.4],
        "the entry zone should overlap the lower front of the car while projecting along local -Z"
    );
    let dismount_anchor = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_dismount"))
        .unwrap();
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(dismount_anchor)
            .unwrap()
            .translation(),
        [0.0, 2.4, -4.6]
    );

    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<CameraXRComponent>(*id)
                .is_some())
            .count(),
        1
    );
    let vehicle_controls = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(id)
                .is_some_and(|gamepad| gamepad.locomotion && gamepad.speed == 1.5)
        })
        .expect("mitten should expose canonical vehicle controls");

    let hands: Vec<_> = world
        .all_components()
        .filter(|id| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(*id)
                .is_some_and(|controller| controller.laser)
        })
        .collect();
    assert_eq!(
        hands.len(),
        2,
        "the mitten player should retain both tracked laser hands"
    );
    for hand in hands {
        let mut pending = vec![hand];
        assert!(
            loop {
                let Some(component) = pending.pop() else {
                    break false;
                };
                if world
                    .get_component_by_id_as::<PointerComponent>(component)
                    .is_some()
                {
                    break true;
                }
                pending.extend(world.children_of(component).iter().copied());
            },
            "each XR controller must retain a pointer without model hand bones"
        );
    }

    let editor = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<EditorComponent>(*id)
                .is_some_and(|editor| editor.active)
        })
        .expect("mittens-corp should author an active editor");
    let mitten = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<GLTFComponent>(*id)
                .is_some_and(|gltf| gltf.uri == "assets/models/mitten.glb")
        })
        .expect("mittens-corp should load mitten");
    assert!(
        world.children_of(mitten).iter().any(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::HandTrackingComponent>(*id)
                .is_some()
        }),
        "mitten should opt in to full finger tracking under its owning GLTF"
    );
    let finger_map = world
        .children_of(mitten)
        .iter()
        .find_map(|id| world.get_component_by_id_as::<HumanoidBoneMapComponent>(*id))
        .unwrap();
    assert_eq!(
        finger_map
            .slots()
            .keys()
            .filter(|slot| {
                let name = slot.as_str();
                ["thumb_", "index_", "middle_", "ring_", "little_"]
                    .iter()
                    .any(|finger| name.contains(finger))
            })
            .count(),
        30,
        "the shared VRoid map must declare every rotating finger bone"
    );
    let mut mitten_ancestor = world.parent_of(mitten);
    while mitten_ancestor.is_some() && mitten_ancestor != Some(editor) {
        mitten_ancestor = mitten_ancestor.and_then(|id| world.parent_of(id));
    }
    assert_eq!(mitten_ancestor, Some(editor));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .is_some()
    }));
    let eye_tracker = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<XREyeTrackingComponent>(id)
                .map(|tracker| (id, tracker))
        })
        .expect("mittens-corp should accept HTC or VRChat OSC eye tracking for blink closure");
    assert!(
        !eye_tracker.1.enable_pupil_direction_tracking,
        "the ambient animation, not live gaze, should own mitten eye direction"
    );
    assert!(world.children_of(mitten).iter().any(|id| {
        world
            .get_component_by_id_as::<HumanoidBoneMapComponent>(*id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AudioInputComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AmplitudeComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<SecondaryMotionComponent>(id)
            .is_some()
    }));
    let bow_rest_constraints: Vec<_> = world
        .all_components()
        .filter(|id| {
            world
                .get_component_by_id_as::<ReturnToRestWhenStillComponent>(*id)
                .is_some()
        })
        .collect();
    assert_eq!(bow_rest_constraints.len(), 2);
    let mut bow_roots = Vec::new();
    for id in bow_rest_constraints {
        let spring = world
            .parent_of(id)
            .and_then(|parent| world.get_component_by_id_as::<SpringBoneComponent>(parent))
            .expect("bow rest constraint should belong to a spring chain");
        bow_roots.push(spring.root.clone());
        let config = world
            .get_component_by_id_as::<ReturnToRestWhenStillComponent>(id)
            .unwrap();
        assert_eq!(config.motion_threshold, 0.028);
        assert_eq!(config.still_for, 0.4);
    }
    for root in ["head_bow.001", "head_bow.009"] {
        assert!(bow_roots.contains(&Some(ComponentRef::Query(format!("[name='{root}']")))));
    }
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<SpringColliderComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<ShadingComponent>(id)
            .is_some_and(|shading| shading.model == ShadingModel::Anime)
    }));
    assert!(world.children_of(mitten).iter().any(|id| {
        world
            .get_component_by_id_as::<PoseCaptureComponent>(*id)
            .is_some_and(|capture| capture.asset_name.as_deref() == Some("mitten"))
    }));

    let editor_panels = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .map(EditorUIComponent::panels)
        })
        .expect("mittens-corp should author EditorUI");
    assert_eq!(
        editor_panels,
        vec![EditorPanel::Settings, EditorPanel::Pose]
    );

    let car_root = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("left_display_car"))
        .unwrap();
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            mountable_id,
            EventSignal::MountStarted {
                rider: ComponentId::default(),
                mountable: mountable_id,
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            vehicle_controls,
            EventSignal::XrAxisChanged {
                source_component: vehicle_controls,
                hand: crate::engine::ecs::component::ControllerHand::Left,
                control: XrAxisControl::LeftStick,
                value: [0.0, 1.0],
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 1.0 },
        ),
    );
    let movement_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        movement_output.errors.is_empty(),
        "{:?}",
        movement_output.errors
    );
    assert!(movement_output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::UpdateTransform {
            component_id,
            translation,
            ..
        } if *component_id == car_root
            && translation[1] == -0.75
            && (translation[0] - -19.0).abs() > 0.01
            && (translation[2] - -1.5).abs() > 0.01
    )));

    // Import only the car and publish its bounds without starting audio or XR.
    let car_model = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_model"))
        .unwrap();
    let car_gltf = world
        .children_of(car_model)
        .iter()
        .copied()
        .find(|&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
        .unwrap();
    let mut gltf_system = crate::engine::ecs::system::GLTFSystem::new();
    let mut renderable_system = crate::engine::ecs::system::RenderableSystem::default();
    let mut skinned = crate::engine::ecs::system::SkinnedMeshSystem::new();
    let mut visuals = VisualWorld::default();
    gltf_system.register_component(car_gltf);
    gltf_system.tick_with_queue(
        &mut world,
        &mut visuals,
        &mut skinned,
        &mut renderable_system,
        &mut queue,
        0.0,
    );
    gltf_system.flush_mesh_imports_only(&mut assets);
    let mut pending = vec![car_model];
    while let Some(node) = pending.pop() {
        pending.extend(world.children_of(node).iter().copied());
        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            renderable_system.register_renderable_from_world(&mut world, &mut visuals, node);
        }
    }
    struct CarUploader(u32);
    impl crate::engine::graphics::MeshUploader for CarUploader {
        fn upload_mesh(
            &mut self,
            _: &crate::engine::graphics::CpuMesh,
        ) -> Result<crate::engine::graphics::MeshHandle, Box<dyn std::error::Error>> {
            self.0 += 1;
            Ok(crate::engine::graphics::MeshHandle(self.0))
        }
    }
    renderable_system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut CarUploader(0),
        &mut queue,
    );
    let model_bounds = crate::engine::ecs::system::bounds_system::BoundsSystem::measure_cached_renderable_subtree_bounds(&world, car_model, |_| false).unwrap();
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 0.0 },
        ),
    );
    let placement_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        placement_output.errors.is_empty(),
        "{:?}",
        placement_output.errors
    );
    let origin = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_laser_origin"))
        .unwrap();
    let origin_translation = placement_output
        .intents
        .iter()
        .find_map(|intent| match intent {
            IntentValue::UpdateTransform {
                component_id,
                translation,
                ..
            } if *component_id == origin => Some(*translation),
            _ => None,
        })
        .expect("model readiness should position the shared muzzle frame");
    assert!((origin_translation[2] - (model_bounds.min[2] - 0.10)).abs() < 1e-4);
    assert!(
        (origin_translation[1]
            - (model_bounds.min[1] + (model_bounds.max[1] - model_bounds.min[1]) * 0.56))
            .abs()
            < 1e-4
    );

    for control in [XrButtonControl::RightGrip, XrButtonControl::RightTrigger] {
        rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                vehicle_controls,
                EventSignal::XrButtonDown {
                    source_component: vehicle_controls,
                    hand: crate::engine::ecs::component::ControllerHand::Right,
                    control,
                    value: 1.0,
                },
            ),
        );
    }
    let fire_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(fire_output.errors.is_empty(), "{:?}", fire_output.errors);
    assert!(fire_output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::SetAnimationState {
            state: crate::engine::ecs::component::AnimationState::Playing,
            ..
        }
    )));

    let active_clips: Vec<_> = fire_output
        .intents
        .iter()
        .filter_map(|intent| match intent {
            IntentValue::SetAnimationState {
                component_id,
                state: crate::engine::ecs::component::AnimationState::Playing,
            } => Some(*component_id),
            _ => None,
        })
        .collect();
    assert_eq!(
        active_clips.len(),
        2,
        "the weapon flash and independent beam each have their own clip"
    );
    let mut animation = crate::engine::ecs::system::AnimationSystem::new();
    for &animation_id in &active_clips {
        animation.register_animation(&mut world, animation_id);
        for keyframe in world.children_of(animation_id).to_vec() {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(
                    keyframe,
                )
                .is_some()
            {
                animation.register_keyframe(&mut world, keyframe);
            }
        }
        animation.set_animation_state(
            animation_id,
            crate::engine::ecs::component::AnimationState::Playing,
        );
    }
    let mut animation_executor =
        crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor {
            session: &mut session,
            render_assets: &mut assets,
            emit: &mut queue,
            benchmarks: &mut Default::default(),
            graphics_settings: &mut Default::default(),
        };
    animation.tick_with_beat_and_executor(
        &mut world,
        0.0,
        60.0,
        &mut rx,
        Some(&mut animation_executor),
    );
    let shot_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(shot_output.errors.is_empty(), "{:?}", shot_output.errors);
    let mut shot_intents = shot_output.intents;
    shot_intents.extend(
        rx.drain_ready_intents()
            .into_iter()
            .filter_map(|signal| signal.intent.map(|intent| intent.value)),
    );
    let named_effect = |name| {
        world
            .all_components()
            .find(|&id| world.component_label(id) == Some(name))
            .unwrap()
    };
    let flash = named_effect("car_laser_muzzle_flash");
    let flight = named_effect("car_laser_flight");
    let beam = named_effect("laser_beam_glow");
    let flash_frame = named_effect("car_laser_muzzle_flash_0");
    assert_eq!(
        world.parent_of(flash),
        Some(origin),
        "muzzle flash must stay attached to the car's weapon"
    );
    assert_eq!(
        world.parent_of(flight),
        None,
        "launched beam must be independent of the car"
    );
    assert!(fire_output.intents.iter().any(|intent|matches!(intent, IntentValue::SetTransformTrs {component_id,space:crate::engine::transform::TransformSpace::World,..} if *component_id==flight)));
    assert!(shot_intents.iter().any(|intent|matches!(intent, IntentValue::UpdateTransform {component_id,scale,..} if *component_id==beam && *scale==[1.;3])));
    assert!(shot_intents.iter().any(|intent|matches!(intent, IntentValue::UpdateTransform {component_id,scale,..} if *component_id==flash_frame && *scale==[0.90;3])));
    let flash_rotation = world
        .get_component_by_id_as::<TransformComponent>(flash)
        .unwrap()
        .transform
        .rotation;
    assert!(
        flash_rotation[0].abs() < 1e-4
            && (flash_rotation[1] - 1.).abs() < 1e-4
            && flash_rotation[2].abs() < 1e-4
            && flash_rotation[3].abs() < 1e-4
    );

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            mountable_id,
            EventSignal::MountEnded {
                rider: ComponentId::default(),
                mountable: mountable_id,
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 1.0 },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            vehicle_controls,
            EventSignal::XrButtonDown {
                source_component: vehicle_controls,
                hand: crate::engine::ecs::component::ControllerHand::Right,
                control: XrButtonControl::RightTrigger,
                value: 1.0,
            },
        ),
    );
    let dismounted_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        dismounted_output.errors.is_empty(),
        "{:?}",
        dismounted_output.errors
    );
    assert!(dismounted_output.intents.iter().all(|intent| !matches!(
        intent,
        IntentValue::UpdateTransform { component_id, .. } if *component_id == car_root
    )));
    assert!(dismounted_output.intents.iter().all(|intent| !matches!(
        intent,
        IntentValue::SetAnimationState {
            state: crate::engine::ecs::component::AnimationState::Playing,
            ..
        }
    )));
}
