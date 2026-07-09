use super::{Component, ComponentRef};
use crate::engine::ecs::{ComponentId, IntentValue};

#[derive(Debug, Clone)]
pub struct ActionComponent {
    /// Runtime intent. ComponentId slots inside are `ComponentId::null()`
    /// placeholders until resolution; after resolution, they're real ids
    /// filled in from `target_sources` in the variant's declaration order.
    pub signal: IntentValue,
    /// Authoring metadata, one entry per ComponentId slot in `signal`,
    /// ordered by the slot's declaration order in the variant. Used by
    /// dump (lossless round-trip) and by resolution (look up ids).
    pub target_sources: Vec<ComponentRef>,
    /// Whether `signal`'s ComponentId slots hold real ids (true) or null
    /// placeholders (false). Set by the resolution pass invoked by
    /// `AnimationSystem` per the owning `AnimationComponent`'s configured
    /// resolve timing.
    pub resolved: bool,
}

impl ActionComponent {
    /// Construct from an already-resolved IntentValue (no ComponentId
    /// targets, or all targets pre-resolved). Use this for built-in /
    /// engine-emitted actions; MMS authoring goes through the registry
    /// which builds with `new_authored` instead.
    pub fn new(signal: IntentValue) -> Self {
        Self {
            signal,
            target_sources: Vec::new(),
            resolved: true,
        }
    }

    /// Construct from a signal whose ComponentId slots are placeholders
    /// plus the authoring sources for each slot (in declaration order).
    /// `resolved` starts false; resolution happens when the owning
    /// `AnimationSystem` first processes this action.
    pub fn new_authored(signal: IntentValue, target_sources: Vec<ComponentRef>) -> Self {
        Self {
            signal,
            target_sources,
            resolved: false,
        }
    }

    pub fn print(message: impl Into<String>) -> Self {
        Self::new(IntentValue::Print {
            message: message.into(),
        })
    }
}

impl Default for ActionComponent {
    fn default() -> Self {
        Self {
            signal: IntentValue::Noop,
            target_sources: Vec::new(),
            resolved: true,
        }
    }
}

// ---------------------------------------------------------------------------
// IntentValue slot enumeration
//
// Used by ActionComponent for: (a) sanity-checking that `target_sources.len()`
// matches the number of ComponentId slots in the variant; (b) reading current
// slot values for dump; (c) writing resolved ids into slots after lookup.
//
// Only covers the variants ActionComponent.signal actually carries. Variants
// the engine emits internally (Register*, intra-system bookkeeping) are not
// authorable from MMS and never appear here — they return zero slots.
// ---------------------------------------------------------------------------

/// Number of ComponentId slots in `signal`'s variant (counts each element
/// of any `Vec<ComponentId>` field).
pub fn signal_target_slot_count(signal: &IntentValue) -> usize {
    use IntentValue::*;
    match signal {
        Noop | Print { .. } => 0,

        SetColor { component_ids, .. }
        | SetText { component_ids, .. }
        | SetEmissiveIntensity { component_ids, .. }
        | SetPosition { component_ids, .. }
        | LookAt { component_ids, .. }
        | GLTFArmatureVisible { component_ids, .. }
        | Detach { component_ids }
        | RemoveSubtree { component_ids }
        | AudioGraphRebuild { component_ids }
        | RequestRaycast { component_ids }
        | AudioLowPassSetCutoffHz { component_ids, .. }
        | AudioBandPassSetCenterHz { component_ids, .. }
        | OscillatorSetEnabled { component_ids, .. }
        | OscillatorSetPitch { component_ids, .. }
        | OscillatorScheduleSetPitch { component_ids, .. }
        | AudioSchedulePlay { component_ids, .. }
        | UpdateTransform { component_ids, .. } => component_ids.len(),

        Attach { parents, .. } | AttachClone { parents, .. } => parents.len() + 1,
        RemoveChild { parents, .. } | RemoveChildren { parents } => parents.len(),

        // Variants ActionComponent never carries — no authored targets.
        _ => 0,
    }
}

/// Apply resolved ids back into `signal`'s ComponentId slots in declaration
/// order. Caller must guarantee `ids.len() == signal_target_slot_count(signal)`.
pub fn apply_resolved_targets(signal: &mut IntentValue, ids: &[ComponentId]) {
    use IntentValue::*;
    let mut cursor = 0usize;
    let mut take = |n: usize| -> &[ComponentId] {
        let slice = &ids[cursor..cursor + n];
        cursor += n;
        slice
    };
    match signal {
        Noop | Print { .. } => {}

        SetColor { component_ids, .. }
        | SetText { component_ids, .. }
        | SetEmissiveIntensity { component_ids, .. }
        | SetPosition { component_ids, .. }
        | LookAt { component_ids, .. }
        | GLTFArmatureVisible { component_ids, .. }
        | Detach { component_ids }
        | RemoveSubtree { component_ids }
        | AudioGraphRebuild { component_ids }
        | RequestRaycast { component_ids }
        | AudioLowPassSetCutoffHz { component_ids, .. }
        | AudioBandPassSetCenterHz { component_ids, .. }
        | OscillatorSetEnabled { component_ids, .. }
        | OscillatorSetPitch { component_ids, .. }
        | OscillatorScheduleSetPitch { component_ids, .. }
        | AudioSchedulePlay { component_ids, .. }
        | UpdateTransform { component_ids, .. } => {
            let n = component_ids.len();
            component_ids.copy_from_slice(take(n));
        }

        Attach { parents, child }
        | AttachClone {
            parents,
            prefab_root: child,
        } => {
            let n = parents.len();
            parents.copy_from_slice(take(n));
            *child = take(1)[0];
        }
        RemoveChild { parents, .. } | RemoveChildren { parents } => {
            let n = parents.len();
            parents.copy_from_slice(take(n));
        }

        _ => {}
    }
    debug_assert_eq!(
        cursor,
        ids.len(),
        "slot count mismatch in apply_resolved_targets"
    );
}

impl Component for ActionComponent {
    fn name(&self) -> &'static str {
        "action"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterAction {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::meow_meow::ast::Expression;

        // Render an ComponentRef back to the surface form the author wrote.
        // Guid → "@uuid:<hex>". Query → the original selector string. Both
        // are Expression::String so the registry's arg_target_source can
        // re-parse them.
        fn target_expr(t: &ComponentRef) -> Expression {
            match t {
                ComponentRef::Guid(u) => Expression::String(format!("@uuid:{u}")),
                ComponentRef::Query(s) => Expression::String(s.clone()),
            }
        }
        // For "vec-of-targets" arg slots: always emit an Array so the
        // dump form matches the registry's expectation
        // (`arg_target_source_vec` accepts arrays or single values, but
        // emitting an array is unambiguous and round-trip-stable).
        let targets_expr = |slice: &[ComponentRef]| -> Expression {
            Expression::Array(slice.iter().map(target_expr).collect())
        };

        match &self.signal {
            IntentValue::Noop => ce_call("Action", "noop", vec![]),
            IntentValue::Print { message } => ce_call("Action", "print", vec![s(message)]),
            IntentValue::SetColor { rgba, .. } => ce_call(
                "Action",
                "set_color",
                vec![
                    targets_expr(&self.target_sources),
                    array(nums(rgba.iter().map(|&v| v as f64))),
                ],
            ),
            IntentValue::SetText { text, .. } => ce_call(
                "Action",
                "set_text",
                vec![targets_expr(&self.target_sources), s(text)],
            ),
            IntentValue::SetEmissiveIntensity { intensity, .. } => ce_call(
                "Action",
                "set_emissive_intensity",
                vec![targets_expr(&self.target_sources), num(*intensity as f64)],
            ),
            IntentValue::SetPosition { position, .. } => ce_call(
                "Action",
                "set_position",
                vec![
                    targets_expr(&self.target_sources),
                    array(nums(position.iter().map(|&v| v as f64))),
                ],
            ),
            IntentValue::Attach { .. } => {
                // target_sources convention: [parents..., child].
                let (parents, child) = self
                    .target_sources
                    .split_at(self.target_sources.len().saturating_sub(1));
                let child_expr = child.first().map(target_expr).unwrap_or_else(|| s(""));
                ce_call("Action", "attach", vec![targets_expr(parents), child_expr])
            }
            IntentValue::AttachClone { .. } => {
                let (parents, prefab) = self
                    .target_sources
                    .split_at(self.target_sources.len().saturating_sub(1));
                let prefab_expr = prefab.first().map(target_expr).unwrap_or_else(|| s(""));
                ce_call(
                    "Action",
                    "attach_clone",
                    vec![targets_expr(parents), prefab_expr],
                )
            }
            IntentValue::Detach { .. } => {
                ce_call("Action", "detach", vec![targets_expr(&self.target_sources)])
            }
            IntentValue::RemoveSubtree { .. } => ce_call(
                "Action",
                "remove_subtree",
                vec![targets_expr(&self.target_sources)],
            ),
            IntentValue::RequestRaycast { .. } => ce_call(
                "Action",
                "request_raycast",
                vec![targets_expr(&self.target_sources)],
            ),
            IntentValue::UpdateTransform {
                translation,
                rotation_quat_xyzw,
                scale,
                ..
            } => {
                // Dump uses the quat form (`update_transform_quat`) for
                // lossless round-trip. The runtime form is always a
                // quaternion; the euler authoring path is one-way.
                let target_expr_ = self
                    .target_sources
                    .first()
                    .map(target_expr)
                    .unwrap_or_else(|| s(""));
                ce_call(
                    "Action",
                    "update_transform_quat",
                    vec![
                        target_expr_,
                        array(nums(translation.iter().map(|&v| v as f64))),
                        array(nums(rotation_quat_xyzw.iter().map(|&v| v as f64))),
                        array(nums(scale.iter().map(|&v| v as f64))),
                    ],
                )
            }
            // Variants ActionComponent shouldn't carry (engine-internal,
            // or not yet wired into the MMS surface). Fall back to a
            // noop so dump still produces parseable output.
            _ => ce_call("Action", "noop", vec![]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::{Key, KeyData};

    fn cid(n: u64) -> ComponentId {
        ComponentId::from(KeyData::from_ffi(n))
    }

    #[test]
    fn slot_count_matches_apply_for_attach_variant() {
        let mut iv = IntentValue::Attach {
            parents: vec![ComponentId::null(), ComponentId::null()],
            child: ComponentId::null(),
        };
        assert_eq!(signal_target_slot_count(&iv), 3);
        apply_resolved_targets(&mut iv, &[cid(10), cid(11), cid(12)]);
        let IntentValue::Attach { parents, child } = iv else {
            unreachable!()
        };
        assert_eq!(parents, vec![cid(10), cid(11)]);
        assert_eq!(child, cid(12));
    }

    #[test]
    fn slot_count_matches_apply_for_vec_only_variant() {
        let mut iv = IntentValue::SetColor {
            component_ids: vec![ComponentId::null(), ComponentId::null()],
            rgba: [1.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(signal_target_slot_count(&iv), 2);
        apply_resolved_targets(&mut iv, &[cid(7), cid(8)]);
        let IntentValue::SetColor { component_ids, .. } = iv else {
            unreachable!()
        };
        assert_eq!(component_ids, vec![cid(7), cid(8)]);
    }

    #[test]
    fn no_slots_for_print() {
        let iv = IntentValue::Print {
            message: "hi".into(),
        };
        assert_eq!(signal_target_slot_count(&iv), 0);
    }
}
