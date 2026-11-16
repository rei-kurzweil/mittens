use super::*;

#[test]
fn amplitude_binding_maps_current_rms_and_primary_driver_wins() {
    let mut world = World::default();
    let amplitude_id = world.add_component(AmplitudeComponent::rolling_window(0.25).unwrap());
    let amplitude_guid = world.get_component_record(amplitude_id).unwrap().guid;
    {
        let amplitude = world
            .get_component_by_id_as_mut::<AmplitudeComponent>(amplitude_id)
            .unwrap();
        amplitude.retained = AmplitudeSample {
            generation: amplitude.generation,
            sequence: 1,
            timestamp_sec: 1.0,
            valid_frames: 64,
            rms: 0.06,
            peak: 0.1,
            status: AmplitudeStatus::Live,
        };
    }
    let avc_id = world.add_component(
        AvatarControlComponent::new()
            .with_mouth_open_from_amplitude(ComponentRef::Guid(amplitude_guid))
            .with_mouth_open_rms_floor(0.02)
            .unwrap()
            .with_mouth_open_rms_ceiling(0.10)
            .unwrap()
            .with_mouth_open_smoothing(0.0)
            .unwrap(),
    );
    let joint = world.add_component(TransformComponent::new());
    let key = MorphTargetKey {
        node_index: 0,
        primitive_index: 0,
        target_index: 0,
    };
    let mut gltf = GLTFComponent::new("synthetic.glb");
    gltf.armature_joint_transforms.push(joint);
    gltf.morph_targets.push(MorphTargetInfo {
        key,
        label: Some("MouthA".into()),
        base_factor: 0.1,
    });
    gltf.morph_factors.insert(
        key,
        MorphFactorState {
            external: None,
            script: None,
            idle_blink: None,
            base: 0.1,
            driver: None,
            amplitude_mouth_open: None,
        },
    );
    let gltf_id = world.add_component(gltf);
    let map_id =
        world.add_component(MorphTargetMapComponent::new().with_slot("viseme_aa", "MouthA"));
    world.add_child(avc_id, gltf_id).unwrap();
    world.add_child(gltf_id, joint).unwrap();
    world.add_child(gltf_id, map_id).unwrap();
    let mut maps = HumanoidBoneMapSystem::default();

    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    let state = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .unwrap()
        .morph_factors[&key];
    assert!((state.amplitude_mouth_open.unwrap() - 0.5).abs() < 1e-6);
    assert!((state.effective() - 0.5).abs() < 1e-6);

    world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf_id)
        .unwrap()
        .morph_factors
        .get_mut(&key)
        .unwrap()
        .driver = Some(0.8);
    let state = world
        .get_component_by_id_as::<GLTFComponent>(gltf_id)
        .unwrap()
        .morph_factors[&key];
    assert_eq!(
        state.effective(),
        0.8,
        "primary viseme/animation driver must win"
    );

    world
        .get_component_by_id_as_mut::<AmplitudeComponent>(amplitude_id)
        .unwrap()
        .bump_generation(AmplitudeStatus::Invalid);
    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    assert_eq!(
        world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .unwrap()
            .morph_factors[&key]
            .amplitude_mouth_open,
        Some(0.0),
    );
}

#[test]
fn amplitude_binding_missing_slot_is_harmless_and_diagnosed_once() {
    let mut world = World::default();
    let amplitude_id = world.add_component(AmplitudeComponent::default());
    let guid = world.get_component_record(amplitude_id).unwrap().guid;
    let avc_id = world.add_component(
        AvatarControlComponent::new()
            .with_mouth_open_from_amplitude(ComponentRef::Guid(guid))
            .with_mouth_open_smoothing(0.0)
            .unwrap(),
    );
    let joint = world.add_component(TransformComponent::new());
    let mut gltf = GLTFComponent::new("synthetic.glb");
    gltf.armature_joint_transforms.push(joint);
    let gltf_id = world.add_component(gltf);
    world.add_child(avc_id, gltf_id).unwrap();
    world.add_child(gltf_id, joint).unwrap();
    let mut maps = HumanoidBoneMapSystem::default();
    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    assert!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .unwrap()
            .mouth_open_missing_slot_diagnosed
    );
}

#[test]
fn amplitude_binding_re_resolves_selector_after_source_replacement() {
    let mut world = World::default();
    let first =
        world.add_component_boxed_named("voice_level", Box::new(AmplitudeComponent::default()));
    let avc_id = world.add_component(
        AvatarControlComponent::new()
            .with_mouth_open_from_amplitude(ComponentRef::Query("#voice_level".into()))
            .with_mouth_open_smoothing(0.0)
            .unwrap(),
    );
    // No rig is required to verify the durable reference cache lifecycle.
    let mut maps = HumanoidBoneMapSystem::default();
    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    assert_eq!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .unwrap()
            .resolved_mouth_open_amplitude,
        Some(first)
    );

    world.remove_component_leaf(first).unwrap();
    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    assert_eq!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .unwrap()
            .resolved_mouth_open_amplitude,
        None
    );

    let replacement =
        world.add_component_boxed_named("voice_level", Box::new(AmplitudeComponent::default()));
    update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
    assert_eq!(
        world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .unwrap()
            .resolved_mouth_open_amplitude,
        Some(replacement)
    );
}
