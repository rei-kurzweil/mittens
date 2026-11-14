use std::collections::BTreeMap;

use crate::engine::ecs::component::Component;
use crate::engine::ecs::{ComponentId, SignalEmitter};

/// Values at or below this magnitude do not enter the GPU active palette.
/// This is deliberately private API until real tracker tuning data exists.
pub(crate) const MORPH_ACTIVE_EPSILON: f32 = 1.0e-4;

/// Engine-owned order for public facial semantic channels. Backends reduce
/// their phonemes to these names; authored maps never expose backend labels.
pub const CANONICAL_MORPH_CHANNELS: &[&str] = &[
    "left_eye_blink",
    "right_eye_blink",
    "viseme_sil",
    "viseme_pp",
    "viseme_ff",
    "viseme_th",
    "viseme_dd",
    "viseme_kk",
    "viseme_ch",
    "viseme_ss",
    "viseme_nn",
    "viseme_rr",
    "viseme_aa",
    "viseme_e",
    "viseme_ih",
    "viseme_oh",
    "viseme_ou",
];

pub fn is_canonical_morph_channel(channel: &str) -> bool {
    CANONICAL_MORPH_CHANNELS.contains(&channel)
}

/// Stable, instance-scoped identity for an imported glTF morph target.
/// Human-readable labels are lookup metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MorphTargetKey {
    pub node_index: usize,
    pub primitive_index: usize,
    pub target_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphTargetInfo {
    pub key: MorphTargetKey,
    pub label: Option<String>,
    pub base_factor: f32,
}

/// Connects one imported primitive renderable to its owning glTF instance.
/// Target indices are intentionally structural, never inferred from labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MorphTargetBindingComponent {
    pub gltf: ComponentId,
    pub node_index: usize,
    pub primitive_index: usize,
}
impl MorphTargetBindingComponent {
    pub fn new(gltf: ComponentId, node_index: usize, primitive_index: usize) -> Self {
        Self {
            gltf,
            node_index,
            primitive_index,
        }
    }
}
impl Component for MorphTargetBindingComponent {
    fn name(&self) -> &'static str {
        "morph_target_binding"
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, _: &mut dyn SignalEmitter, _: ComponentId) {}
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        crate::engine::ecs::component::ce_helpers::ce_call("MorphTargetBinding", "new", vec![])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MorphFactorState {
    /// An explicit external binding owns the final value until released.
    pub external: Option<f32>,
    /// Authored idle blink fallback; live expression/tracking drivers take priority.
    pub idle_blink: Option<f32>,
    pub base: f32,
    /// Primary animation/viseme owner. When present it wins over fallbacks.
    pub driver: Option<f32>,
    /// AVC-owned, explicitly authored amplitude fallback.
    pub amplitude_mouth_open: Option<f32>,
}
impl MorphFactorState {
    pub fn new(base: f32) -> Self {
        Self {
            external: None,
            base,
            driver: None,
            amplitude_mouth_open: None,
            idle_blink: None,
        }
    }
    pub fn effective(self) -> f32 {
        self.external
            .or(self.driver)
            .or(self.amplitude_mouth_open)
            .unwrap_or_else(|| {
                self.idle_blink
                    .filter(|v| *v > 0.)
                    .map_or(self.base, |v| self.base.max(v))
            })
    }
    pub fn is_active(self) -> bool {
        self.effective().abs() > MORPH_ACTIVE_EPSILON
    }
}

/// Builds the per-instance GPU palette input. Ordering by stable key makes
/// uploads deterministic and lets a renderer retain a range across frames.
pub(crate) fn active_factors<'a>(
    factors: impl Iterator<Item = (&'a MorphTargetKey, &'a MorphFactorState)>,
) -> Vec<(MorphTargetKey, f32)> {
    factors
        .filter_map(|(key, state)| {
            let value = state.effective();
            (value.abs() > MORPH_ACTIVE_EPSILON).then_some((*key, value))
        })
        .collect()
}

/// Explicit semantic mapping owned by a GLTF instance.
#[derive(Debug, Clone, Default)]
pub struct MorphTargetMapComponent {
    slots: BTreeMap<String, String>,
}
impl MorphTargetMapComponent {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_slot(mut self, channel: impl Into<String>, label: impl Into<String>) -> Self {
        self.slots.insert(channel.into(), label.into());
        self
    }
    pub fn slot(self, channel: &str, label: &str) -> Result<Self, String> {
        if !is_canonical_morph_channel(channel) {
            return Err(format!("unknown MorphTargetMap channel '{channel}'"));
        }
        Ok(self.with_slot(channel, label))
    }
    pub fn slots(&self) -> &BTreeMap<String, String> {
        &self.slots
    }
}
impl Component for MorphTargetMapComponent {
    fn name(&self) -> &'static str {
        "morph_target_map"
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, _: &mut dyn SignalEmitter, _: ComponentId) {}
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        self.slots.iter().fold(
            ce_call("MorphTargetMap", "new", vec![]),
            |out, (channel, label)| out.with_call("slot", vec![s(channel), s(label)]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn active_threshold_is_strict_and_signed() {
        let key = MorphTargetKey {
            node_index: 1,
            primitive_index: 2,
            target_index: 3,
        };
        let mut factors = BTreeMap::new();
        factors.insert(
            key,
            MorphFactorState {
                external: None,
                idle_blink: None,
                base: MORPH_ACTIVE_EPSILON,
                driver: None,
                amplitude_mouth_open: None,
            },
        );
        assert!(active_factors(factors.iter()).is_empty());
        factors.insert(
            key,
            MorphFactorState {
                external: None,
                idle_blink: None,
                base: -MORPH_ACTIVE_EPSILON * 1.01,
                driver: None,
                amplitude_mouth_open: None,
            },
        );
        assert_eq!(
            active_factors(factors.iter()),
            vec![(key, -MORPH_ACTIVE_EPSILON * 1.01)]
        );
    }

    #[test]
    fn driver_does_not_destroy_base_factor() {
        let state = MorphFactorState {
            external: None,
            idle_blink: None,
            base: 0.25,
            driver: Some(0.8),
            amplitude_mouth_open: Some(0.5),
        };
        assert_eq!(state.effective(), 0.8);
        assert_eq!(
            MorphFactorState {
                external: None,
                idle_blink: None,
                driver: None,
                amplitude_mouth_open: None,
                ..state
            }
            .effective(),
            0.25
        );
    }

    #[test]
    fn canonical_viseme_channels_are_valid_map_slots() {
        let map = MorphTargetMapComponent::new()
            .slot("viseme_aa", "Fcl_MTH_A")
            .unwrap();
        assert_eq!(map.slots().get("viseme_aa"), Some(&"Fcl_MTH_A".to_owned()));
        assert!(
            MorphTargetMapComponent::new()
                .slot("backend_aa", "A")
                .is_err()
        );
    }
}
