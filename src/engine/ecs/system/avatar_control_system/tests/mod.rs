//! Avatar control system tests, grouped by responsibility.
use super::*;
use crate::engine::ecs::component::xr_eye_tracking::{EyeClosureSample, EyeGazeSample};
use crate::engine::ecs::component::{
    AmplitudeComponent, AmplitudeSample, AmplitudeStatus, ComponentRef, EyeRotationLimits,
    MorphFactorState, MorphTargetInfo, MorphTargetKey, MorphTargetMapComponent,
    RestAttachmentComponent, XREyeTrackingComponent, XREyeTrackingHtcComponent,
};
use crate::engine::ecs::system::{
    HumanoidSlotProvenance, HumanoidSlotReport, HumanoidSlotStatus, ResolvedHumanoidTarget,
    ResolvedHumanoidTargetKind,
};
use crate::engine::ecs::{EventSignal, IntentSignal};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct RecordingEmitter(Vec<[f32; 4]>);

impl SignalEmitter for RecordingEmitter {
    fn push_event(&mut self, _scope: ComponentId, _event: EventSignal) {}

    fn push_intent(&mut self, _scope: ComponentId, intent: IntentSignal) {
        if let IntentValue::UpdateTransform {
            rotation_quat_xyzw, ..
        } = intent.value
        {
            self.0.push(rotation_quat_xyzw);
        }
    }
}

pub(super) fn left_arm_report(
    upper: ComponentId,
    lower: ComponentId,
    hand: ComponentId,
) -> HumanoidBoneMapReport {
    let mut slots = BTreeMap::new();
    for (slot, component) in [
        (HumanoidSlot::LeftUpperArm, upper),
        (HumanoidSlot::LeftLowerArm, lower),
        (HumanoidSlot::LeftHand, hand),
    ] {
        slots.insert(
            slot,
            HumanoidSlotReport {
                slot,
                status: HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                    component,
                    kind: ResolvedHumanoidTargetKind::SkinJoint,
                }),
                provenance: HumanoidSlotProvenance::ConventionName,
                diagnostic: None,
            },
        );
    }
    HumanoidBoneMapReport {
        owning_gltf: ComponentId::default(),
        source: None,
        generation: 1,
        valid: true,
        diagnostics: Vec::new(),
        slots,
    }
}


mod arms;
mod capsule;
mod eyes;
mod hands;
mod head;
mod mouth;
