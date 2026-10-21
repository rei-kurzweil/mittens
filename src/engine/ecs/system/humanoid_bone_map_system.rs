use std::collections::{BTreeMap, HashMap, HashSet};

use crate::engine::ecs::component::{
    AuthoredSlot, BoneRestPoseComponent, ComponentRef, GLTFComponent, HumanoidBoneMapComponent,
    HumanoidSlot, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::utils::math::{mat4_identity, mat4_mul};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedHumanoidTargetKind {
    SkinJoint,
    Transform,
    GeneratedAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedHumanoidTarget {
    pub component: ComponentId,
    pub kind: ResolvedHumanoidTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanoidSlotProvenance {
    Explicit,
    ConventionName,
    TopologyGeometry,
    Symmetry,
    DerivedEyeMidpoint,
    HeadFallback,
    Absent,
    Unresolved,
    Ambiguous,
    InvalidExplicit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanoidSlotStatus {
    Resolved(ResolvedHumanoidTarget),
    Absent,
    Unresolved,
    Ambiguous { candidates: Vec<ComponentId> },
    InvalidExplicit(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanoidSlotReport {
    pub slot: HumanoidSlot,
    pub status: HumanoidSlotStatus,
    pub provenance: HumanoidSlotProvenance,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanoidBoneMapReport {
    pub owning_gltf: ComponentId,
    pub source: Option<ComponentId>,
    pub generation: u64,
    pub valid: bool,
    pub diagnostics: Vec<String>,
    pub slots: BTreeMap<HumanoidSlot, HumanoidSlotReport>,
}

impl HumanoidBoneMapReport {
    pub fn target(&self, slot: HumanoidSlot) -> Option<ResolvedHumanoidTarget> {
        match &self.slots.get(&slot)?.status {
            HumanoidSlotStatus::Resolved(target) => Some(*target),
            _ => None,
        }
    }
    pub fn slot(&self, slot: HumanoidSlot) -> Option<&HumanoidSlotReport> {
        self.slots.get(&slot)
    }
    /// Head is the only global AVC prerequisite. Each arm is independently usable.
    pub fn head_ready(&self) -> bool {
        self.target(HumanoidSlot::Head).is_some()
    }
    pub fn arm_ready(&self, left: bool) -> bool {
        let slots = if left {
            [
                HumanoidSlot::LeftUpperArm,
                HumanoidSlot::LeftLowerArm,
                HumanoidSlot::LeftHand,
            ]
        } else {
            [
                HumanoidSlot::RightUpperArm,
                HumanoidSlot::RightLowerArm,
                HumanoidSlot::RightHand,
            ]
        };
        slots.into_iter().all(|slot| self.target(slot).is_some())
    }
    pub fn lower_body_ready(&self) -> bool {
        [
            HumanoidSlot::Hips,
            HumanoidSlot::LeftUpperLeg,
            HumanoidSlot::LeftLowerLeg,
            HumanoidSlot::LeftFoot,
            HumanoidSlot::RightUpperLeg,
            HumanoidSlot::RightLowerLeg,
            HumanoidSlot::RightFoot,
        ]
        .into_iter()
        .all(|slot| self.target(slot).is_some())
    }
}

#[derive(Debug, Default)]
pub struct HumanoidBoneMapSystem {
    reports: HashMap<ComponentId, HumanoidBoneMapReport>,
    sources: HashSet<ComponentId>,
    implicit_requests: HashSet<ComponentId>,
    generated_camera_anchors: HashMap<ComponentId, ComponentId>,
    generations: HashMap<ComponentId, u64>,
}

impl HumanoidBoneMapSystem {
    pub fn install_handlers(rx: &mut RxWorld) {
        rx.add_global_handler_named(
            SignalKind::GltfInitialized,
            Some("humanoid_bone_map_gltf_initialized".into()),
            gltf_initialized_handler,
        );
        rx.add_global_handler_named(
            SignalKind::ParentChanged,
            Some("humanoid_bone_map_topology_changed".into()),
            topology_changed_handler,
        );
    }
    pub fn report(&self, gltf: ComponentId) -> Option<&HumanoidBoneMapReport> {
        self.reports.get(&gltf)
    }
    pub fn reports(&self) -> impl Iterator<Item = &HumanoidBoneMapReport> {
        self.reports.values()
    }

    /// Explicitly request a retained report for a non-AVC consumer. GLTFs are otherwise
    /// left unscanned unless they own an authored map.
    pub fn request_report(
        &mut self,
        world: &mut World,
        gltf: ComponentId,
    ) -> Option<&HumanoidBoneMapReport> {
        if !self.reports.contains_key(&gltf) {
            self.resolve_gltf(world, gltf, true);
        }
        self.reports.get(&gltf)
    }

    pub fn register_component(&mut self, world: &mut World, source: ComponentId) {
        self.sources.insert(source);
        if let Some(owner) = nearest_gltf(world, source) {
            self.resolve_gltf(world, owner, false);
        }
    }

    pub fn unregister_component(&mut self, world: &mut World, source: ComponentId) {
        self.sources.remove(&source);
        let owner = self
            .reports
            .iter()
            .find_map(|(&owner, report)| (report.source == Some(source)).then_some(owner))
            .or_else(|| nearest_gltf(world, source));
        if let Some(owner) = owner {
            self.invalidate(owner);
        }
    }

    pub fn component_removed(&mut self, world: &World, component: ComponentId) {
        self.sources.remove(&component);
        self.implicit_requests.remove(&component);
        self.generated_camera_anchors.remove(&component);
        if self.reports.remove(&component).is_some() {
            return;
        }
        let affected: Vec<_> = self.reports.iter().filter_map(|(&owner, report)| {
            (report.source == Some(component) || report.slots.values().any(|s| matches!(s.status, HumanoidSlotStatus::Resolved(t) if t.component == component))).then_some(owner)
        }).collect();
        for owner in affected {
            self.invalidate(owner);
        }
        let _ = world;
    }

    pub fn gltf_initialized(&mut self, world: &mut World, gltf: ComponentId) {
        let requested = self
            .sources
            .iter()
            .any(|&source| nearest_gltf(world, source) == Some(gltf))
            || self.reports.contains_key(&gltf)
            || self.implicit_requests.contains(&gltf);
        if requested {
            self.resolve_gltf(world, gltf, false);
        }
    }

    pub fn topology_changed(&mut self, world: &mut World, component: ComponentId) {
        let affected: Vec<_> = self
            .reports
            .iter()
            .filter_map(|(&owner, report)| {
                let moved_report_member = report.source.is_some_and(|source| {
                    source == component || is_ancestor(world, component, source)
                }) || report.slots.values().any(|slot| {
                    matches!(slot.status, HumanoidSlotStatus::Resolved(target)
                        if target.component == component
                            || is_ancestor(world, component, target.component))
                });
                moved_report_member.then_some(owner)
            })
            .collect();
        for owner in affected {
            self.invalidate(owner);
        }
        if let Some(gltf) = nearest_gltf(world, component) {
            if self.reports.contains_key(&gltf)
                || self.implicit_requests.contains(&gltf)
                || self
                    .sources
                    .iter()
                    .any(|&source| nearest_gltf(world, source) == Some(gltf))
            {
                self.resolve_gltf(world, gltf, false);
            }
        }
    }

    /// Request the implicit Auto map used by AVC when no authored map exists.
    pub fn request_for_avc(&mut self, world: &mut World, avc: ComponentId) -> Option<ComponentId> {
        let gltf = first_skinned_gltf_below(world, avc)?;
        if !self.reports.contains_key(&gltf) {
            self.resolve_gltf(world, gltf, true);
        }
        Some(gltf)
    }

    /// Re-resolve a retained report. This is event-driven; callers invoke it at registration,
    /// GLTF initialization, or a relevant topology change, never from a frame scan.
    pub fn resolve_gltf(&mut self, world: &mut World, gltf_id: ComponentId, implicit_auto: bool) {
        if implicit_auto {
            self.implicit_requests.insert(gltf_id);
        }
        let retained_generated_camera = self
            .generated_camera_anchors
            .get(&gltf_id)
            .copied()
            .or_else(|| {
                self.reports.get(&gltf_id).and_then(|report| {
                    report
                        .target(HumanoidSlot::CameraAnchor)
                        .filter(|target| target.kind == ResolvedHumanoidTargetKind::GeneratedAnchor)
                        .map(|target| target.component)
                })
            });
        let generation = self.generations.entry(gltf_id).or_default();
        *generation = generation.wrapping_add(1).max(1);
        let generation = *generation;
        let map_sources: Vec<_> = self
            .sources
            .iter()
            .copied()
            .filter(|&source| nearest_gltf(world, source) == Some(gltf_id))
            .collect();
        if map_sources.len() > 1 {
            self.reports.insert(
                gltf_id,
                HumanoidBoneMapReport {
                    owning_gltf: gltf_id,
                    source: None,
                    generation,
                    valid: false,
                    diagnostics: vec![
                        "multiple HumanoidBoneMap components belong to one GLTF".into(),
                    ],
                    slots: unresolved_slots(),
                },
            );
            return;
        }
        let source = map_sources.first().copied();
        if source.is_none()
            && !implicit_auto
            && !self.implicit_requests.contains(&gltf_id)
            && !self.reports.contains_key(&gltf_id)
        {
            return;
        }
        let map = source
            .and_then(|id| {
                world
                    .get_component_by_id_as::<HumanoidBoneMapComponent>(id)
                    .cloned()
            })
            .unwrap_or_default();
        let Some(gltf) = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .cloned()
        else {
            return;
        };
        let joints: HashSet<_> = gltf.armature_joint_transforms.iter().copied().collect();
        let mut slots = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for slot in HumanoidSlot::ALL {
            let report = match map.authored(slot) {
                AuthoredSlot::Absent => slot_report(
                    slot,
                    HumanoidSlotStatus::Absent,
                    HumanoidSlotProvenance::Absent,
                    None,
                ),
                AuthoredSlot::Reference(reference) => {
                    resolve_explicit(world, gltf_id, &joints, slot, &reference)
                }
                AuthoredSlot::Unspecified if map.automap => infer_by_name(world, &gltf, slot),
                AuthoredSlot::Unspecified => slot_report(
                    slot,
                    HumanoidSlotStatus::Unresolved,
                    HumanoidSlotProvenance::Unresolved,
                    None,
                ),
            };
            slots.insert(slot, report);
        }
        validate_arm_topology(world, &mut slots, true);
        validate_arm_topology(world, &mut slots, false);
        resolve_camera(
            world,
            gltf_id,
            &gltf,
            &map,
            &mut slots,
            &mut diagnostics,
            retained_generated_camera,
        );
        if let Some(target) =
            slots
                .get(&HumanoidSlot::CameraAnchor)
                .and_then(|slot| match slot.status {
                    HumanoidSlotStatus::Resolved(target)
                        if target.kind == ResolvedHumanoidTargetKind::GeneratedAnchor =>
                    {
                        Some(target)
                    }
                    _ => None,
                })
        {
            self.generated_camera_anchors
                .insert(gltf_id, target.component);
        }
        let valid = diagnostics.is_empty()
            && !slots
                .values()
                .any(|entry| matches!(entry.status, HumanoidSlotStatus::InvalidExplicit(_)))
            && slots
                .get(&HumanoidSlot::Head)
                .is_some_and(|entry| matches!(entry.status, HumanoidSlotStatus::Resolved(_)));
        self.reports.insert(
            gltf_id,
            HumanoidBoneMapReport {
                owning_gltf: gltf_id,
                source,
                generation,
                valid,
                diagnostics,
                slots,
            },
        );
    }

    fn invalidate(&mut self, gltf: ComponentId) {
        self.reports.remove(&gltf);
        let generation = self.generations.entry(gltf).or_default();
        *generation = generation.wrapping_add(1).max(1);
    }
}

fn gltf_initialized_handler(_world: &mut World, emit: &mut dyn SignalEmitter, signal: &Signal) {
    if let Some(EventSignal::GltfInitialized { gltf, .. }) = signal.event.as_ref() {
        emit.push_intent_now(
            *gltf,
            IntentValue::HumanoidBoneMapGltfInitialized {
                component_id: *gltf,
            },
        );
    }
}

fn topology_changed_handler(_world: &mut World, emit: &mut dyn SignalEmitter, signal: &Signal) {
    if let Some(EventSignal::ParentChanged { child, .. }) = signal.event.as_ref() {
        emit.push_intent_now(
            *child,
            IntentValue::HumanoidBoneMapTopologyChanged {
                component_id: *child,
            },
        );
    }
}

fn slot_report(
    slot: HumanoidSlot,
    status: HumanoidSlotStatus,
    provenance: HumanoidSlotProvenance,
    diagnostic: Option<String>,
) -> HumanoidSlotReport {
    HumanoidSlotReport {
        slot,
        status,
        provenance,
        diagnostic,
    }
}

fn unresolved_slots() -> BTreeMap<HumanoidSlot, HumanoidSlotReport> {
    HumanoidSlot::ALL
        .into_iter()
        .map(|slot| {
            (
                slot,
                slot_report(
                    slot,
                    HumanoidSlotStatus::Unresolved,
                    HumanoidSlotProvenance::Unresolved,
                    None,
                ),
            )
        })
        .collect()
}

fn resolve_explicit(
    world: &World,
    gltf: ComponentId,
    joints: &HashSet<ComponentId>,
    slot: HumanoidSlot,
    reference: &ComponentRef,
) -> HumanoidSlotReport {
    let matches: Vec<_> = match reference {
        ComponentRef::Guid(guid) => world.component_id_by_guid(*guid).into_iter().collect(),
        ComponentRef::Query(query) => world
            .scripting_query_roots(gltf)
            .into_iter()
            .flat_map(|root| world.find_all_components(root, query))
            .collect(),
    };
    let mut matches: Vec<_> = matches
        .into_iter()
        .filter(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(*id)
                .is_some()
                && (slot == HumanoidSlot::CameraAnchor || joints.contains(id))
        })
        .collect();
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [id] => slot_report(
            slot,
            HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                component: *id,
                kind: if joints.contains(id) {
                    ResolvedHumanoidTargetKind::SkinJoint
                } else {
                    ResolvedHumanoidTargetKind::Transform
                },
            }),
            HumanoidSlotProvenance::Explicit,
            None,
        ),
        [] => {
            let message = format!(
                "explicit {} selector did not uniquely resolve to {}",
                slot,
                if slot == HumanoidSlot::CameraAnchor {
                    "a Transform"
                } else {
                    "a skin joint in the owning GLTF"
                }
            );
            slot_report(
                slot,
                HumanoidSlotStatus::InvalidExplicit(message.clone()),
                HumanoidSlotProvenance::InvalidExplicit,
                Some(message),
            )
        }
        ids => {
            let message = format!(
                "explicit {slot} selector matched {} eligible transforms",
                ids.len()
            );
            slot_report(
                slot,
                HumanoidSlotStatus::InvalidExplicit(message.clone()),
                HumanoidSlotProvenance::InvalidExplicit,
                Some(message),
            )
        }
    }
}

fn infer_by_name(world: &World, gltf: &GLTFComponent, slot: HumanoidSlot) -> HumanoidSlotReport {
    if slot == HumanoidSlot::CameraAnchor {
        return slot_report(
            slot,
            HumanoidSlotStatus::Unresolved,
            HumanoidSlotProvenance::Unresolved,
            None,
        );
    }
    let patterns = slot_patterns(slot);
    let mut matches = Vec::new();
    for &joint in &gltf.armature_joint_transforms {
        let label = world.component_label(joint).unwrap_or_default();
        let tokens = name_tokens(label);
        if is_helper(&tokens) {
            continue;
        }
        if patterns
            .iter()
            .any(|pattern| convention_tokens_match(&tokens, pattern))
        {
            matches.push(joint);
        }
    }
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [id] => slot_report(
            slot,
            HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                component: *id,
                kind: ResolvedHumanoidTargetKind::SkinJoint,
            }),
            HumanoidSlotProvenance::ConventionName,
            None,
        ),
        [] => slot_report(
            slot,
            HumanoidSlotStatus::Unresolved,
            HumanoidSlotProvenance::Unresolved,
            None,
        ),
        ids => slot_report(
            slot,
            HumanoidSlotStatus::Ambiguous {
                candidates: ids.to_vec(),
            },
            HumanoidSlotProvenance::Ambiguous,
            Some(format!("{} equally strong name matches", ids.len())),
        ),
    }
}

fn convention_tokens_match(tokens: &[String], pattern: &[String]) -> bool {
    if tokens == pattern {
        return true;
    }
    let Some(prefix) = tokens.strip_suffix(pattern) else {
        return false;
    };
    !prefix.is_empty()
        && prefix.iter().all(|token| {
            matches!(
                token.as_str(),
                "mixamorig" | "armature" | "skeleton" | "rig"
            )
        })
}

fn name_tokens(name: &str) -> Vec<String> {
    let mut separated = String::with_capacity(name.len() * 2);
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() && prev_lower {
            separated.push('_');
        }
        if ch.is_ascii_alphanumeric() {
            separated.push(ch.to_ascii_lowercase());
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            separated.push('_');
            prev_lower = false;
        }
    }
    separated
        .split('_')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn p(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

fn slot_patterns(slot: HumanoidSlot) -> Vec<Vec<String>> {
    use HumanoidSlot::*;
    let lr = |side: &str, bone: &[&str]| {
        let mut a = vec![side.to_owned()];
        a.extend(bone.iter().map(|s| (*s).to_owned()));
        let short = if side == "left" { "l" } else { "r" };
        let mut b = vec![short.to_owned()];
        b.extend(bone.iter().map(|s| (*s).to_owned()));
        vec![a, b]
    };
    match slot {
        Root => vec![p(&["root"]), p(&["armature"])],
        Hips => vec![p(&["hips"]), p(&["pelvis"]), p(&["j", "bip", "c", "hips"])],
        Spine => vec![
            p(&["spine"]),
            p(&["spine", "1"]),
            p(&["j", "bip", "c", "spine"]),
        ],
        Chest => vec![
            p(&["chest"]),
            p(&["spine", "2"]),
            p(&["j", "bip", "c", "chest"]),
        ],
        UpperChest => vec![
            p(&["upper", "chest"]),
            p(&["spine", "3"]),
            p(&["j", "bip", "c", "upper", "chest"]),
        ],
        Neck => vec![p(&["neck"]), p(&["j", "bip", "c", "neck"])],
        Head => vec![p(&["head"]), p(&["j", "bip", "c", "head"])],
        LeftShoulder => lr("left", &["shoulder"]),
        RightShoulder => lr("right", &["shoulder"]),
        LeftUpperArm => {
            let mut v = lr("left", &["upper", "arm"]);
            v.push(p(&["j", "bip", "l", "upper", "arm"]));
            v.push(p(&["left", "arm"]));
            v
        }
        RightUpperArm => {
            let mut v = lr("right", &["upper", "arm"]);
            v.push(p(&["j", "bip", "r", "upper", "arm"]));
            v.push(p(&["right", "arm"]));
            v
        }
        LeftLowerArm => {
            let mut v = lr("left", &["lower", "arm"]);
            v.push(p(&["j", "bip", "l", "lower", "arm"]));
            v.push(p(&["left", "fore", "arm"]));
            v.push(p(&["left", "forearm"]));
            v
        }
        RightLowerArm => {
            let mut v = lr("right", &["lower", "arm"]);
            v.push(p(&["j", "bip", "r", "lower", "arm"]));
            v.push(p(&["right", "fore", "arm"]));
            v.push(p(&["right", "forearm"]));
            v
        }
        LeftHand => {
            let mut v = lr("left", &["hand"]);
            v.push(p(&["j", "bip", "l", "hand"]));
            v.push(p(&["left", "wrist"]));
            v
        }
        RightHand => {
            let mut v = lr("right", &["hand"]);
            v.push(p(&["j", "bip", "r", "hand"]));
            v.push(p(&["right", "wrist"]));
            v
        }
        LeftEye => {
            let mut v = lr("left", &["eye"]);
            v.push(p(&["j", "adj", "l", "face", "eye"]));
            v
        }
        RightEye => {
            let mut v = lr("right", &["eye"]);
            v.push(p(&["j", "adj", "r", "face", "eye"]));
            v
        }
        LeftMiddleProximal => {
            let mut v = lr("left", &["middle", "proximal"]);
            v.push(p(&["j", "bip", "l", "middle1"]));
            v.push(p(&["left", "hand", "middle1"]));
            v
        }
        RightMiddleProximal => {
            let mut v = lr("right", &["middle", "proximal"]);
            v.push(p(&["j", "bip", "r", "middle1"]));
            v.push(p(&["right", "hand", "middle1"]));
            v
        }
        LeftMiddleDistal => {
            let mut v = lr("left", &["middle", "distal"]);
            v.push(p(&["j", "bip", "l", "middle3"]));
            v.push(p(&["left", "hand", "middle3"]));
            v
        }
        RightMiddleDistal => {
            let mut v = lr("right", &["middle", "distal"]);
            v.push(p(&["j", "bip", "r", "middle3"]));
            v.push(p(&["right", "hand", "middle3"]));
            v
        }
        LeftIndexProximal => {
            let mut v = lr("left", &["index", "proximal"]);
            v.push(p(&["j", "bip", "l", "index1"]));
            v.push(p(&["left", "hand", "index1"]));
            v
        }
        RightIndexProximal => {
            let mut v = lr("right", &["index", "proximal"]);
            v.push(p(&["j", "bip", "r", "index1"]));
            v.push(p(&["right", "hand", "index1"]));
            v
        }
        LeftLittleProximal => {
            let mut v = lr("left", &["little", "proximal"]);
            v.push(p(&["j", "bip", "l", "little1"]));
            v.push(p(&["left", "hand", "pinky1"]));
            v
        }
        RightLittleProximal => {
            let mut v = lr("right", &["little", "proximal"]);
            v.push(p(&["j", "bip", "r", "little1"]));
            v.push(p(&["right", "hand", "pinky1"]));
            v
        }
        // Complete finger articulation is explicit in this slice; expand inference separately.
        LeftThumbMetacarpal
        | LeftThumbProximal
        | LeftThumbDistal
        | LeftIndexIntermediate
        | LeftIndexDistal
        | LeftMiddleIntermediate
        | LeftRingProximal
        | LeftRingIntermediate
        | LeftRingDistal
        | LeftLittleIntermediate
        | LeftLittleDistal
        | RightThumbMetacarpal
        | RightThumbProximal
        | RightThumbDistal
        | RightIndexIntermediate
        | RightIndexDistal
        | RightMiddleIntermediate
        | RightRingProximal
        | RightRingIntermediate
        | RightRingDistal
        | RightLittleIntermediate
        | RightLittleDistal => vec![],
        LeftUpperLeg => {
            let mut v = lr("left", &["upper", "leg"]);
            v.push(p(&["j", "bip", "l", "upper", "leg"]));
            v.push(p(&["left", "up", "leg"]));
            v
        }
        RightUpperLeg => {
            let mut v = lr("right", &["upper", "leg"]);
            v.push(p(&["j", "bip", "r", "upper", "leg"]));
            v.push(p(&["right", "up", "leg"]));
            v
        }
        LeftLowerLeg => {
            let mut v = lr("left", &["lower", "leg"]);
            v.push(p(&["j", "bip", "l", "lower", "leg"]));
            v.push(p(&["left", "leg"]));
            v
        }
        RightLowerLeg => {
            let mut v = lr("right", &["lower", "leg"]);
            v.push(p(&["j", "bip", "r", "lower", "leg"]));
            v.push(p(&["right", "leg"]));
            v
        }
        LeftFoot => {
            let mut v = lr("left", &["foot"]);
            v.push(p(&["j", "bip", "l", "foot"]));
            v
        }
        RightFoot => {
            let mut v = lr("right", &["foot"]);
            v.push(p(&["j", "bip", "r", "foot"]));
            v
        }
        LeftToes => {
            let mut v = lr("left", &["toes"]);
            v.push(p(&["left", "toe", "base"]));
            v
        }
        RightToes => {
            let mut v = lr("right", &["toes"]);
            v.push(p(&["right", "toe", "base"]));
            v
        }
        CameraAnchor => vec![],
    }
}

fn is_helper(tokens: &[String]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "helper"
                | "twist"
                | "collider"
                | "adjust"
                | "adjustment"
                | "secondary"
                | "spring"
                | "end"
        )
    })
}

fn validate_arm_topology(
    world: &World,
    slots: &mut BTreeMap<HumanoidSlot, HumanoidSlotReport>,
    left: bool,
) {
    let (upper, lower, hand) = if left {
        (
            HumanoidSlot::LeftUpperArm,
            HumanoidSlot::LeftLowerArm,
            HumanoidSlot::LeftHand,
        )
    } else {
        (
            HumanoidSlot::RightUpperArm,
            HumanoidSlot::RightLowerArm,
            HumanoidSlot::RightHand,
        )
    };
    let ids = [upper, lower, hand].map(|slot| {
        slots.get(&slot).and_then(|entry| match entry.status {
            HumanoidSlotStatus::Resolved(t) => Some(t.component),
            _ => None,
        })
    });
    if let [Some(upper_id), Some(lower_id), Some(hand_id)] = ids {
        if !is_ancestor(world, upper_id, lower_id) || !is_ancestor(world, lower_id, hand_id) {
            for slot in [upper, lower, hand] {
                if slots
                    .get(&slot)
                    .is_some_and(|entry| entry.provenance != HumanoidSlotProvenance::Explicit)
                {
                    slots.insert(
                        slot,
                        slot_report(
                            slot,
                            HumanoidSlotStatus::Unresolved,
                            HumanoidSlotProvenance::Unresolved,
                            Some("name match failed arm topology validation".into()),
                        ),
                    );
                }
            }
        }
    }
}

fn is_ancestor(world: &World, ancestor: ComponentId, mut child: ComponentId) -> bool {
    for _ in 0..32 {
        let Some(parent) = world.parent_of(child) else {
            return false;
        };
        if parent == ancestor {
            return true;
        }
        child = parent;
    }
    false
}

fn resolve_camera(
    world: &mut World,
    gltf_id: ComponentId,
    gltf: &GLTFComponent,
    map: &HumanoidBoneMapComponent,
    slots: &mut BTreeMap<HumanoidSlot, HumanoidSlotReport>,
    diagnostics: &mut Vec<String>,
    retained_generated_camera: Option<ComponentId>,
) {
    if matches!(
        map.authored(HumanoidSlot::CameraAnchor),
        AuthoredSlot::Reference(_)
    ) && slots
        .get(&HumanoidSlot::CameraAnchor)
        .is_some_and(|s| matches!(s.status, HumanoidSlotStatus::Resolved(_)))
    {
        return;
    }
    let explicit_invalid = slots
        .get(&HumanoidSlot::CameraAnchor)
        .is_some_and(|s| matches!(s.status, HumanoidSlotStatus::InvalidExplicit(_)));
    if explicit_invalid {
        diagnostics.push("explicit camera_anchor is invalid; operational fallback selected".into());
    }
    if map.automap {
        let head = resolved_id(slots, HumanoidSlot::Head);
        let named: Vec<_> = gltf
            .spawned_node_transforms
            .iter()
            .copied()
            .filter(|id| {
                let tokens = name_tokens(world.component_label(*id).unwrap_or_default());
                world
                    .get_component_by_id_as::<TransformComponent>(*id)
                    .is_some()
                    && tokens
                        .iter()
                        .any(|t| t == "camera" || t == "view" || t == "eyes")
                    && head.is_none_or(|head| *id == head || is_ancestor(world, head, *id))
            })
            .collect();
        if named.len() == 1 {
            set_camera(
                slots,
                named[0],
                ResolvedHumanoidTargetKind::Transform,
                HumanoidSlotProvenance::ConventionName,
                explicit_invalid,
            );
            return;
        }
    }
    if let (Some(left), Some(right), Some(head)) = (
        resolved_id(slots, HumanoidSlot::LeftEye),
        resolved_id(slots, HumanoidSlot::RightEye),
        resolved_id(slots, HumanoidSlot::Head),
    ) {
        if let (Some(a), Some(b)) = (
            rest_world_matrix(world, gltf_id, left),
            rest_world_matrix(world, gltf_id, right),
        ) {
            let midpoint = [
                (a[3][0] + b[3][0]) * 0.5,
                (a[3][1] + b[3][1]) * 0.5,
                (a[3][2] + b[3][2]) * 0.5,
            ];
            let head_m = rest_world_matrix(world, gltf_id, head).unwrap_or_else(mat4_identity);
            let local = [
                midpoint[0] - head_m[3][0],
                midpoint[1] - head_m[3][1],
                midpoint[2] - head_m[3][2],
            ];
            let anchor = retained_generated_camera
                .filter(|anchor| {
                    world
                        .get_component_by_id_as::<TransformComponent>(*anchor)
                        .is_some()
                })
                .unwrap_or_else(|| {
                    let anchor = world.add_component(TransformComponent::new());
                    let marker = world.add_component(SerializeComponent::off());
                    let _ = world.set_parent(marker, Some(anchor));
                    anchor
                });
            if let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(anchor)
            {
                *transform = TransformComponent::new().with_position(local[0], local[1], local[2]);
            }
            if world.parent_of(anchor) != Some(head) {
                let _ = world.set_parent(anchor, Some(head));
            }
            set_camera(
                slots,
                anchor,
                ResolvedHumanoidTargetKind::GeneratedAnchor,
                HumanoidSlotProvenance::DerivedEyeMidpoint,
                explicit_invalid,
            );
            return;
        }
    }
    if let Some(head) = resolved_id(slots, HumanoidSlot::Head) {
        set_camera(
            slots,
            head,
            ResolvedHumanoidTargetKind::SkinJoint,
            HumanoidSlotProvenance::HeadFallback,
            explicit_invalid,
        );
    }
}

fn set_camera(
    slots: &mut BTreeMap<HumanoidSlot, HumanoidSlotReport>,
    id: ComponentId,
    kind: ResolvedHumanoidTargetKind,
    provenance: HumanoidSlotProvenance,
    preserve_invalid: bool,
) {
    let diagnostic = preserve_invalid.then(|| {
        format!(
            "explicit camera_anchor is invalid; using operational {:?} fallback at {id:?}",
            provenance
        )
    });
    slots.insert(
        HumanoidSlot::CameraAnchor,
        slot_report(
            HumanoidSlot::CameraAnchor,
            HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                component: id,
                kind,
            }),
            provenance,
            diagnostic,
        ),
    );
}

fn resolved_id(
    slots: &BTreeMap<HumanoidSlot, HumanoidSlotReport>,
    slot: HumanoidSlot,
) -> Option<ComponentId> {
    match slots.get(&slot)?.status {
        HumanoidSlotStatus::Resolved(target) => Some(target.component),
        _ => None,
    }
}

fn rest_world_matrix(world: &World, stop: ComponentId, id: ComponentId) -> Option<[[f32; 4]; 4]> {
    let mut chain = Vec::new();
    let mut current = Some(id);
    while let Some(node) = current {
        if node == stop {
            break;
        }
        chain.push(node);
        current = world.parent_of(node);
    }
    let mut out = mat4_identity();
    for node in chain.into_iter().rev() {
        let local = if let Some(rest) = world
            .children_of(node)
            .iter()
            .find_map(|child| world.get_component_by_id_as::<BoneRestPoseComponent>(*child))
        {
            let mut transform = crate::engine::graphics::primitives::Transform::default();
            transform.translation = rest.translation;
            transform.rotation = rest.rotation;
            transform.scale = rest.scale;
            transform.recompute_model();
            transform.model
        } else {
            world
                .get_component_by_id_as::<TransformComponent>(node)?
                .transform
                .model
        };
        out = mat4_mul(out, local);
    }
    Some(out)
}

fn nearest_gltf(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    for _ in 0..128 {
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some() {
            return Some(id);
        }
        id = world.parent_of(id)?;
    }
    None
}

fn first_skinned_gltf_below(world: &World, root: ComponentId) -> Option<ComponentId> {
    world
        .find_all_components(root, "GLTF")
        .into_iter()
        .find(|id| {
            world
                .get_component_by_id_as::<GLTFComponent>(*id)
                .is_some_and(|g| !g.armature_joint_transforms.is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::TransformComponent;

    fn rig(names: &[&str]) -> (World, ComponentId, Vec<ComponentId>) {
        let mut world = World::default();
        let root = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new("synthetic.glb"));
        world.add_child(root, gltf).unwrap();
        let mut ids = Vec::new();
        for &name in names {
            let id = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
            world.add_child(root, id).unwrap();
            ids.push(id);
        }
        world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap()
            .armature_joint_transforms = ids.clone();
        world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap()
            .spawned_node_transforms = ids.clone();
        (world, gltf, ids)
    }

    #[test]
    fn exact_tokenization_rejects_numbered_misleading_head() {
        let (mut world, gltf, _) = rig(&["J_Bip_C_Head.001"]);
        let mut system = HumanoidBoneMapSystem::default();
        system.resolve_gltf(&mut world, gltf, true);
        assert!(
            system
                .report(gltf)
                .unwrap()
                .target(HumanoidSlot::Head)
                .is_none()
        );
    }

    #[test]
    fn explicit_absence_and_auto_disabled_win() {
        let (mut world, gltf, ids) = rig(&["Head", "LeftHand"]);
        let map = world.add_component(
            HumanoidBoneMapComponent::new()
                .with_slot(
                    HumanoidSlot::Head,
                    ComponentRef::Guid(world.get_component_record(ids[0]).unwrap().guid),
                )
                .with_absent(HumanoidSlot::Neck)
                .with_automap_disabled(),
        );
        world.add_child(gltf, map).unwrap();
        let mut system = HumanoidBoneMapSystem::default();
        system.sources.insert(map);
        system.resolve_gltf(&mut world, gltf, false);
        let report = system.report(gltf).unwrap();
        assert_eq!(report.target(HumanoidSlot::Head).unwrap().component, ids[0]);
        assert!(matches!(
            report.slot(HumanoidSlot::Neck).unwrap().status,
            HumanoidSlotStatus::Absent
        ));
        assert!(report.target(HumanoidSlot::LeftHand).is_none());
    }

    #[test]
    fn duplicate_exact_names_remain_ambiguous() {
        let (mut world, gltf, ids) = rig(&["Head", "Head"]);
        let mut system = HumanoidBoneMapSystem::default();
        system.resolve_gltf(&mut world, gltf, true);
        assert!(matches!(
            &system
                .report(gltf)
                .unwrap()
                .slot(HumanoidSlot::Head)
                .unwrap()
                .status,
            HumanoidSlotStatus::Ambiguous { candidates } if candidates == &ids
        ));
    }

    #[test]
    fn missing_explicit_camera_keeps_head_fallback_and_rebinds_on_attachment() {
        let (mut world, gltf, ids) = rig(&["Head"]);
        let map = world.add_component(HumanoidBoneMapComponent::new().with_slot(
            HumanoidSlot::CameraAnchor,
            ComponentRef::Query("#camera_socket".into()),
        ));
        world.add_child(gltf, map).unwrap();
        let mut system = HumanoidBoneMapSystem::default();
        system.sources.insert(map);
        system.resolve_gltf(&mut world, gltf, false);
        let first = system.report(gltf).unwrap().clone();
        assert_eq!(
            first.target(HumanoidSlot::CameraAnchor).unwrap().component,
            ids[0]
        );
        assert!(!first.valid);

        let socket =
            world.add_component_boxed_named("camera_socket", Box::new(TransformComponent::new()));
        world.add_child(gltf, socket).unwrap();
        system.topology_changed(&mut world, socket);
        let rebound = system.report(gltf).unwrap();
        assert_eq!(
            rebound
                .target(HumanoidSlot::CameraAnchor)
                .unwrap()
                .component,
            socket
        );
        assert!(rebound.generation > first.generation);
        assert!(rebound.valid);
    }

    #[test]
    fn multiple_authored_maps_invalidate_the_owner() {
        let (mut world, gltf, _) = rig(&["Head"]);
        let a = world.add_component(HumanoidBoneMapComponent::new());
        let b = world.add_component(HumanoidBoneMapComponent::new());
        world.add_child(gltf, a).unwrap();
        world.add_child(gltf, b).unwrap();
        let mut system = HumanoidBoneMapSystem::default();
        system.sources.extend([a, b]);
        system.resolve_gltf(&mut world, gltf, false);
        assert!(!system.report(gltf).unwrap().valid);
    }

    #[test]
    fn mixamo_namespace_is_accepted_and_arm_topology_is_validated() {
        let (mut world, gltf, ids) = rig(&[
            "mixamorig:Head",
            "mixamorig:LeftArm",
            "mixamorig:LeftForeArm",
            "mixamorig:LeftHand",
        ]);
        world.set_parent(ids[2], Some(ids[1])).unwrap();
        world.set_parent(ids[3], Some(ids[2])).unwrap();
        let mut system = HumanoidBoneMapSystem::default();
        system.resolve_gltf(&mut world, gltf, true);
        let report = system.report(gltf).unwrap();
        assert_eq!(report.target(HumanoidSlot::Head).unwrap().component, ids[0]);
        assert!(report.arm_ready(true));
    }

    #[test]
    fn helper_and_twist_names_are_not_anatomical_candidates() {
        let (mut world, gltf, ids) = rig(&["HeadTwist", "Head"]);
        let mut system = HumanoidBoneMapSystem::default();
        system.resolve_gltf(&mut world, gltf, true);
        assert_eq!(
            system
                .report(gltf)
                .unwrap()
                .target(HumanoidSlot::Head)
                .unwrap()
                .component,
            ids[1]
        );
    }

    #[test]
    fn nonhumanoid_joint_inventory_does_not_produce_a_valid_avatar() {
        let (mut world, gltf, _) = rig(&["Wheel", "Propeller", "Collider"]);
        let mut system = HumanoidBoneMapSystem::default();
        system.resolve_gltf(&mut world, gltf, true);
        let report = system.report(gltf).unwrap();
        assert!(!report.valid);
        assert!(!report.head_ready());
    }

    #[test]
    fn paired_eyes_generate_one_retained_midpoint_anchor() {
        let (mut world, gltf, ids) = rig(&["Head", "LeftEye", "RightEye"]);
        world.set_parent(ids[1], Some(ids[0])).unwrap();
        world.set_parent(ids[2], Some(ids[0])).unwrap();
        *world
            .get_component_by_id_as_mut::<TransformComponent>(ids[1])
            .unwrap() = TransformComponent::new().with_position(-0.03, 0.08, 0.07);
        *world
            .get_component_by_id_as_mut::<TransformComponent>(ids[2])
            .unwrap() = TransformComponent::new().with_position(0.03, 0.08, 0.07);
        let mut system = HumanoidBoneMapSystem::default();
        system.resolve_gltf(&mut world, gltf, true);
        let first = system
            .report(gltf)
            .unwrap()
            .target(HumanoidSlot::CameraAnchor)
            .unwrap();
        assert_eq!(first.kind, ResolvedHumanoidTargetKind::GeneratedAnchor);
        let position = world
            .get_component_by_id_as::<TransformComponent>(first.component)
            .unwrap()
            .transform
            .translation;
        assert!((position[0]).abs() < 1.0e-6);
        assert!((position[1] - 0.08).abs() < 1.0e-6);
        assert!((position[2] - 0.07).abs() < 1.0e-6);

        system.resolve_gltf(&mut world, gltf, true);
        assert_eq!(
            system
                .report(gltf)
                .unwrap()
                .target(HumanoidSlot::CameraAnchor)
                .unwrap()
                .component,
            first.component
        );
    }
}
