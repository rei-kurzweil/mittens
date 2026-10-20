use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::engine::ecs::component::{Component, ComponentRef};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter};

/// A semantic landmark in a humanoid armature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HumanoidSlot {
    Root,
    Hips,
    Spine,
    Chest,
    UpperChest,
    Neck,
    Head,
    LeftShoulder,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    RightShoulder,
    RightUpperArm,
    RightLowerArm,
    RightHand,
    LeftThumbMetacarpal,
    LeftThumbProximal,
    LeftThumbDistal,
    LeftIndexProximal,
    LeftIndexIntermediate,
    LeftIndexDistal,
    LeftMiddleProximal,
    LeftMiddleIntermediate,
    LeftMiddleDistal,
    LeftRingProximal,
    LeftRingIntermediate,
    LeftRingDistal,
    LeftLittleProximal,
    LeftLittleIntermediate,
    LeftLittleDistal,
    RightThumbMetacarpal,
    RightThumbProximal,
    RightThumbDistal,
    RightIndexProximal,
    RightIndexIntermediate,
    RightIndexDistal,
    RightMiddleProximal,
    RightMiddleIntermediate,
    RightMiddleDistal,
    RightRingProximal,
    RightRingIntermediate,
    RightRingDistal,
    RightLittleProximal,
    RightLittleIntermediate,
    RightLittleDistal,
    LeftEye,
    RightEye,
    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    LeftToes,
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
    RightToes,
    CameraAnchor,
}

impl HumanoidSlot {
    pub const ALL: [Self; 56] = [
        Self::Root,
        Self::Hips,
        Self::Spine,
        Self::Chest,
        Self::UpperChest,
        Self::Neck,
        Self::Head,
        Self::LeftShoulder,
        Self::LeftUpperArm,
        Self::LeftLowerArm,
        Self::LeftHand,
        Self::RightShoulder,
        Self::RightUpperArm,
        Self::RightLowerArm,
        Self::RightHand,
        Self::LeftThumbMetacarpal,
        Self::LeftThumbProximal,
        Self::LeftThumbDistal,
        Self::LeftIndexProximal,
        Self::LeftIndexIntermediate,
        Self::LeftIndexDistal,
        Self::LeftMiddleProximal,
        Self::LeftMiddleIntermediate,
        Self::LeftMiddleDistal,
        Self::LeftRingProximal,
        Self::LeftRingIntermediate,
        Self::LeftRingDistal,
        Self::LeftLittleProximal,
        Self::LeftLittleIntermediate,
        Self::LeftLittleDistal,
        Self::RightThumbMetacarpal,
        Self::RightThumbProximal,
        Self::RightThumbDistal,
        Self::RightIndexProximal,
        Self::RightIndexIntermediate,
        Self::RightIndexDistal,
        Self::RightMiddleProximal,
        Self::RightMiddleIntermediate,
        Self::RightMiddleDistal,
        Self::RightRingProximal,
        Self::RightRingIntermediate,
        Self::RightRingDistal,
        Self::RightLittleProximal,
        Self::RightLittleIntermediate,
        Self::RightLittleDistal,
        Self::LeftEye,
        Self::RightEye,
        Self::LeftUpperLeg,
        Self::LeftLowerLeg,
        Self::LeftFoot,
        Self::LeftToes,
        Self::RightUpperLeg,
        Self::RightLowerLeg,
        Self::RightFoot,
        Self::RightToes,
        Self::CameraAnchor,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Hips => "hips",
            Self::Spine => "spine",
            Self::Chest => "chest",
            Self::UpperChest => "upper_chest",
            Self::Neck => "neck",
            Self::Head => "head",
            Self::LeftShoulder => "left_shoulder",
            Self::LeftUpperArm => "left_upper_arm",
            Self::LeftLowerArm => "left_lower_arm",
            Self::LeftHand => "left_hand",
            Self::RightShoulder => "right_shoulder",
            Self::RightUpperArm => "right_upper_arm",
            Self::RightLowerArm => "right_lower_arm",
            Self::RightHand => "right_hand",
            Self::LeftThumbMetacarpal => "left_thumb_metacarpal",
            Self::LeftThumbProximal => "left_thumb_proximal",
            Self::LeftThumbDistal => "left_thumb_distal",
            Self::LeftIndexProximal => "left_index_proximal",
            Self::LeftIndexIntermediate => "left_index_intermediate",
            Self::LeftIndexDistal => "left_index_distal",
            Self::LeftMiddleProximal => "left_middle_proximal",
            Self::LeftMiddleIntermediate => "left_middle_intermediate",
            Self::LeftMiddleDistal => "left_middle_distal",
            Self::LeftRingProximal => "left_ring_proximal",
            Self::LeftRingIntermediate => "left_ring_intermediate",
            Self::LeftRingDistal => "left_ring_distal",
            Self::LeftLittleProximal => "left_little_proximal",
            Self::LeftLittleIntermediate => "left_little_intermediate",
            Self::LeftLittleDistal => "left_little_distal",
            Self::RightThumbMetacarpal => "right_thumb_metacarpal",
            Self::RightThumbProximal => "right_thumb_proximal",
            Self::RightThumbDistal => "right_thumb_distal",
            Self::RightIndexProximal => "right_index_proximal",
            Self::RightIndexIntermediate => "right_index_intermediate",
            Self::RightIndexDistal => "right_index_distal",
            Self::RightMiddleProximal => "right_middle_proximal",
            Self::RightMiddleIntermediate => "right_middle_intermediate",
            Self::RightMiddleDistal => "right_middle_distal",
            Self::RightRingProximal => "right_ring_proximal",
            Self::RightRingIntermediate => "right_ring_intermediate",
            Self::RightRingDistal => "right_ring_distal",
            Self::RightLittleProximal => "right_little_proximal",
            Self::RightLittleIntermediate => "right_little_intermediate",
            Self::RightLittleDistal => "right_little_distal",
            Self::LeftEye => "left_eye",
            Self::RightEye => "right_eye",
            Self::LeftUpperLeg => "left_upper_leg",
            Self::LeftLowerLeg => "left_lower_leg",
            Self::LeftFoot => "left_foot",
            Self::LeftToes => "left_toes",
            Self::RightUpperLeg => "right_upper_leg",
            Self::RightLowerLeg => "right_lower_leg",
            Self::RightFoot => "right_foot",
            Self::RightToes => "right_toes",
            Self::CameraAnchor => "camera_anchor",
        }
    }
}

impl fmt::Display for HumanoidSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for HumanoidSlot {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim().to_ascii_lowercase().replace('-', "_");
        HumanoidSlot::ALL
            .into_iter()
            .find(|slot| slot.as_str() == value)
            .ok_or_else(|| format!("unknown humanoid slot '{value}'"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredSlot {
    Unspecified,
    Reference(ComponentRef),
    Absent,
}

/// GLTF-owned semantic humanoid declaration. Unspecified slots are inferred by default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanoidBoneMapComponent {
    pub automap: bool,
    slots: BTreeMap<HumanoidSlot, AuthoredSlot>,
    component_id: Option<ComponentId>,
}

impl HumanoidBoneMapComponent {
    pub fn new() -> Self {
        Self {
            automap: true,
            slots: BTreeMap::new(),
            component_id: None,
        }
    }
    pub fn authored(&self, slot: HumanoidSlot) -> AuthoredSlot {
        self.slots
            .get(&slot)
            .cloned()
            .unwrap_or(AuthoredSlot::Unspecified)
    }
    pub fn slots(&self) -> &BTreeMap<HumanoidSlot, AuthoredSlot> {
        &self.slots
    }
    pub fn with_slot(mut self, slot: HumanoidSlot, reference: ComponentRef) -> Self {
        self.slots.insert(slot, AuthoredSlot::Reference(reference));
        self
    }
    pub fn with_absent(mut self, slot: HumanoidSlot) -> Self {
        self.slots.insert(slot, AuthoredSlot::Absent);
        self
    }
    pub fn with_automap_disabled(mut self) -> Self {
        self.automap = false;
        self
    }
    pub fn slot(self, name: &str, reference: ComponentRef) -> Result<Self, String> {
        Ok(self.with_slot(name.parse()?, reference))
    }
    pub fn absent(self, name: &str) -> Result<Self, String> {
        Ok(self.with_absent(name.parse()?))
    }
    pub fn automap_disable(self) -> Self {
        self.with_automap_disabled()
    }
}

impl Default for HumanoidBoneMapComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for HumanoidBoneMapComponent {
    fn name(&self) -> &'static str {
        "humanoid_bone_map"
    }
    fn set_id(&mut self, id: ComponentId) {
        self.component_id = Some(id);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::RegisterHumanoidBoneMap {
                component_id: component,
            },
        );
    }
    fn cleanup(&mut self, emit: &mut dyn SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            IntentValue::UnregisterHumanoidBoneMap {
                component_id: component,
            },
        );
    }
    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut out = ce_call("HumanoidBoneMap", "new", vec![]);
        for (slot, authored) in &self.slots {
            match authored {
                AuthoredSlot::Reference(reference) => {
                    let surface = match reference {
                        ComponentRef::Guid(g) => format!("@uuid:{g}"),
                        ComponentRef::Query(q) => q.clone(),
                    };
                    out = out.with_call("slot", vec![s(slot.as_str()), s(&surface)]);
                }
                AuthoredSlot::Absent => out = out.with_call("absent", vec![s(slot.as_str())]),
                AuthoredSlot::Unspecified => {}
            }
        }
        if !self.automap {
            out = out.with_call("automap_disable", vec![]);
        }
        out
    }
}
