use super::*;
use crate::engine::ecs::component::xr_eye_tracking::{EyeClosureSample, EyeGazeSample};
use crate::engine::ecs::component::{
    MorphFactorState, MorphTargetInfo, MorphTargetKey, MorphTargetMapComponent,
};

#[test]
fn idle_blink_tracks_closure_not_gaze_and_preserves_base_and_live_open_zero() {
    let mut world = World::default();
    let avc = world.add_component(AvatarControlComponent::default());
    let tracker = world.add_component(XREyeTrackingComponent::on());
    world.add_child(avc, tracker).unwrap();
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(tracker)
        .unwrap()
        .gaze_sample = EyeGazeSample {
        left: Some([0., 0., -1.]),
        right: Some([0., 0., -1.]),
        sequence: 1,
    };
    assert_eq!(
        newest_direct_eye_closure(&world, avc),
        (None, None),
        "gaze alone cannot suppress fallback blinking"
    );
    let left = MorphTargetKey {
        node_index: 0,
        primitive_index: 0,
        target_index: 0,
    };
    let right = MorphTargetKey {
        target_index: 1,
        ..left
    };
    let mut model = GLTFComponent::new("not-loaded.glb");
    for (key, label, base) in [(left, "BlinkLeft", 0.1), (right, "BlinkRight", 0.2)] {
        model.morph_targets.push(MorphTargetInfo {
            key,
            label: Some(label.into()),
            base_factor: base,
        });
        model.morph_factors.insert(key, MorphFactorState::new(base));
    }
    let gltf = world.add_component(model);
    let map = world.add_component(
        MorphTargetMapComponent::new()
            .with_slot("left_eye_blink", "BlinkLeft")
            .with_slot("right_eye_blink", "BlinkRight"),
    );
    world.add_child(gltf, map).unwrap();
    let mut state = AvatarControlComponent::default();
    // Reach the peak of a smooth 0.6-second idle cycle.
    state.eye_blink_state.update([None, None], 0.48);
    let amount = state.eye_blink_state.amount(0.6, 0.18);
    assert!(amount > 0.99);
    apply_factors(&mut world, gltf, (None, None), Some(amount));
    assert!(
        world
            .get_component_by_id_as::<GLTFComponent>(gltf)
            .unwrap()
            .morph_factors[&left]
            .effective()
            > 0.99
    );
    world
        .get_component_by_id_as_mut::<XREyeTrackingComponent>(tracker)
        .unwrap()
        .closure_sample = EyeClosureSample {
        left: Some(0.),
        right: None,
        sequence: 2,
    };
    let raw = newest_direct_eye_closure(&world, avc);
    let retained = state.eye_blink_state.update([raw.0, raw.1], 0.);
    assert_eq!(state.eye_blink_state.tracked(), [true, false]);
    apply_factors(&mut world, gltf, (retained[0], retained[1]), Some(amount));
    let model = world.get_component_by_id_as::<GLTFComponent>(gltf).unwrap();
    assert_eq!(
        model.morph_factors[&left].effective(),
        0.,
        "tracked open wins over idle closure"
    );
    assert!(
        model.morph_factors[&right].effective() > 0.99,
        "untracked eye can still blink"
    );
    assert_eq!(model.morph_factors[&left].base, 0.1);
    assert_eq!(
        state.eye_blink_state.update([None, None], 0.10),
        [Some(0.), None],
        "packet gaps hold tracking ownership"
    );
    assert_eq!(
        state.eye_blink_state.update([None, None], 0.16),
        [None, None],
        "sustained loss releases it"
    );
    apply_factors(&mut world, gltf, (None, None), None);
    assert_eq!(
        world
            .get_component_by_id_as::<GLTFComponent>(gltf)
            .unwrap()
            .morph_factors[&left]
            .effective(),
        0.1,
        "disabling fallback restores authored base"
    );
    let mut other = AvatarControlComponent::default();
    other.eye_blink_state.update([None, None], 0.);
    assert_eq!(
        other.eye_blink_state.amount(0.6, 0.18),
        0.,
        "avatars have independent clocks"
    );
}
