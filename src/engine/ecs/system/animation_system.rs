use std::collections::{BTreeMap, BTreeSet};

use crate::engine::ecs::component::{
    ActionComponent, AnimationComponent, AnimationState, KeyframeComponent, MusicNoteComponent,
    QueryRootMode, ResolveTargetsMode,
    action::{apply_resolved_targets, signal_target_slot_count},
    resolve_component_ref,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::animation_system_evaluator::AnimationEvaluator;
use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::meow_meow::evaluator::{RuntimeClosureExecMode, eval_runtime_closure};

#[derive(Debug, Default)]
struct AnimationRuntime {
    keyframes: Vec<ComponentId>,
    fired_keyframes: BTreeSet<ComponentId>,
    /// For audio lookahead scheduling, track the last loop-cycle index each keyframe was
    /// scheduled for.
    audio_scheduled_cycle_by_keyframe: BTreeMap<ComponentId, u64>,
    /// Loop cycle index for audio scheduling. Increments whenever a looping animation wraps.
    audio_cycle: u64,
    start_beat: f64,
    pending_state: Option<AnimationState>,
    /// For `ResolveTargetsMode::OnAttach`: set once the first tick has
    /// bulk-resolved every ActionComponent target under this animation's
    /// keyframes. `OnPlay` mode ignores this and resolves per-action lazily
    /// just before each push.
    attach_resolved: bool,
}

/// Resolve `ActionComponent::target_sources` into concrete ComponentIds and
/// write them into the matching ComponentId slots of `signal`. Idempotent —
/// returns Ok immediately if the action is already resolved.
///
/// Lookup rules:
/// - `ComponentRef::Guid(uuid)` → `world.component_id_by_guid` (O(1)).
/// - `ComponentRef::Query(selector)` → resolved via the shared scoped-query
///   helper. Bare selectors are rooted at `Animation.scope(...)` when present,
///   otherwise the owning action subtree. Explicit `../...` and `/...`
///   prefixes override that base root.
/// Emit one `AudioSchedulePlay` per `MusicNoteComponent` child of `kf_id`.
/// `beat_context` is the absolute beat the note should fire at — set by
/// the audio lookahead pass to the keyframe's global beat, or by the
/// realtime pass to the current beat.
fn fire_music_note_children(
    world: &mut World,
    rx: &mut RxWorld,
    kf_id: ComponentId,
    beat_context: Option<f64>,
) {
    let note_ids: Vec<ComponentId> = world
        .children_of(kf_id)
        .iter()
        .copied()
        .filter(|&cid| {
            world
                .get_component_by_id_as::<MusicNoteComponent>(cid)
                .is_some()
        })
        .collect();

    for note_cid in note_ids {
        // Snapshot the note value (clone is cheap — MusicNote is small).
        let note = match world.get_component_by_id_as::<MusicNoteComponent>(note_cid) {
            Some(mn) => mn.note,
            None => continue,
        };
        rx.push_intent_now(
            note_cid,
            IntentValue::AudioSchedulePlay {
                component_ids: vec![note_cid],
                beat_offset: 0.0,
                beat_context,
                note: Some(note),
                gain: None,
                rate: None,
                duration: None,
            },
        );
    }
}

fn owning_animation_of(world: &World, id: ComponentId) -> Option<ComponentId> {
    let mut cursor = Some(id);
    while let Some(node) = cursor {
        if world
            .get_component_by_id_as::<AnimationComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cursor = world.parent_of(node);
    }
    None
}

fn resolve_animation_scope(world: &mut World, animation_id: ComponentId) -> Option<ComponentId> {
    let (resolved_scope, scope_source) = {
        let animation = world.get_component_by_id_as::<AnimationComponent>(animation_id)?;
        (animation.resolved_scope, animation.scope_source.clone())
    };

    if let Some(scope) = resolved_scope {
        return Some(scope);
    }

    let source = scope_source?;
    let scope = resolve_component_ref(
        world,
        &source,
        Some(animation_id),
        QueryRootMode::SelfSubtree,
    )?;
    let animation = world.get_component_by_id_as_mut::<AnimationComponent>(animation_id)?;
    animation.resolved_scope = Some(scope);
    Some(scope)
}

fn resolve_action_targets(world: &mut World, action_id: ComponentId) -> Result<(), String> {
    let (sources, expected_slots) = {
        let Some(action) = world.get_component_by_id_as::<ActionComponent>(action_id) else {
            return Err(format!("resolve: action {action_id:?} missing"));
        };
        if action.resolved {
            return Ok(());
        }
        (
            action.target_sources.clone(),
            signal_target_slot_count(&action.signal),
        )
    };

    if sources.len() != expected_slots {
        return Err(format!(
            "resolve: action {action_id:?} has {} target_sources but signal expects {} slots",
            sources.len(),
            expected_slots
        ));
    }

    let resolution_root = owning_animation_of(world, action_id)
        .and_then(|animation_id| resolve_animation_scope(world, animation_id))
        .unwrap_or(action_id);

    let mut resolved: Vec<ComponentId> = Vec::with_capacity(sources.len());
    for source in &sources {
        let id = resolve_component_ref(
            world,
            source,
            Some(resolution_root),
            QueryRootMode::SelfSubtree,
        )
        .ok_or_else(|| format!("resolve: source {source:?} matched nothing"))?;
        resolved.push(id);
    }

    let Some(action) = world.get_component_by_id_as_mut::<ActionComponent>(action_id) else {
        return Err(format!(
            "resolve: action {action_id:?} vanished during resolve"
        ));
    };
    apply_resolved_targets(&mut action.signal, &resolved);
    action.resolved = true;
    Ok(())
}

#[derive(Debug, Default)]
pub struct AnimationSystem {
    /// Runtime state keyed by `AnimationComponent` id.
    ///
    /// BTree* gives deterministic iteration order (nice for debugging/logs).
    animations: BTreeMap<ComponentId, AnimationRuntime>,
    last_beat: f64,

    evaluator: AnimationEvaluator,
}

impl AnimationSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_animation(&mut self, world: &mut World, component: ComponentId) {
        if world
            .get_component_by_id_as::<AnimationComponent>(component)
            .is_none()
        {
            return;
        }

        self.animations
            .entry(component)
            .or_insert_with(AnimationRuntime::default);
    }

    pub fn set_animation_state(&mut self, animation: ComponentId, state: AnimationState) {
        self.animations
            .entry(animation)
            .or_insert_with(AnimationRuntime::default)
            .pending_state = Some(state);
    }

    pub fn register_keyframe(&mut self, world: &mut World, component: ComponentId) {
        if world
            .get_component_by_id_as::<KeyframeComponent>(component)
            .is_none()
        {
            return;
        }

        // Find ancestor AnimationComponent.
        let mut cursor = world.parent_of(component);
        while let Some(node) = cursor {
            if world
                .get_component_by_id_as::<AnimationComponent>(node)
                .is_some()
            {
                let runtime = self
                    .animations
                    .entry(node)
                    .or_insert_with(AnimationRuntime::default);
                let list = &mut runtime.keyframes;

                if !list.contains(&component) {
                    list.push(component);
                }

                // Keep deterministic order by beat.
                list.sort_by(|a, b| {
                    let ba = world
                        .get_component_by_id_as::<KeyframeComponent>(*a)
                        .map(|k| k.beat)
                        .unwrap_or(0.0);
                    let bb = world
                        .get_component_by_id_as::<KeyframeComponent>(*b)
                        .map(|k| k.beat)
                        .unwrap_or(0.0);
                    ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
                });
                return;
            }
            cursor = world.parent_of(node);
        }
    }

    pub fn tick_with_beat(&mut self, world: &mut World, beat_now: f64, bpm: f64, rx: &mut RxWorld) {
        // If time jumps backwards, reset fired state.
        if beat_now + 1e-9 < self.last_beat {
            for runtime in self.animations.values_mut() {
                runtime.fired_keyframes.clear();
                runtime.audio_scheduled_cycle_by_keyframe.clear();
                runtime.audio_cycle = 0;
            }
        }

        // Apply any requested state changes.
        // Setting Playing/Looping is treated as a restart.
        for (&anim, runtime) in self.animations.iter_mut() {
            let Some(state) = runtime.pending_state.take() else {
                continue;
            };

            let Some(anim_comp) = world.get_component_by_id_as_mut::<AnimationComponent>(anim)
            else {
                continue;
            };

            anim_comp.state = state;
            runtime.start_beat = beat_now;
            runtime.fired_keyframes.clear();
            runtime.audio_scheduled_cycle_by_keyframe.clear();
            runtime.audio_cycle = 0;
        }

        // Drive animations.
        for (&anim, runtime) in self.animations.iter_mut() {
            let (state, resolve_mode, length_override) =
                match world.get_component_by_id_as::<AnimationComponent>(anim) {
                    Some(c) => (c.state, c.resolve_targets, c.length_beats),
                    None => continue,
                };

            if state == AnimationState::Paused {
                continue;
            }

            if runtime.keyframes.is_empty() {
                continue;
            }

            // OnAttach: on first tick, eagerly resolve every action under this
            // animation's keyframes. Errors are logged but don't halt the
            // animation — individual broken actions just skip in the push
            // path below. OnPlay defers per-action to the push site.
            if matches!(resolve_mode, ResolveTargetsMode::OnAttach) && !runtime.attach_resolved {
                let action_ids: Vec<ComponentId> = runtime
                    .keyframes
                    .iter()
                    .flat_map(|&kf| {
                        world
                            .children_of(kf)
                            .iter()
                            .copied()
                            .filter(|&cid| {
                                world
                                    .get_component_by_id_as::<ActionComponent>(cid)
                                    .is_some()
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                for action_cid in action_ids {
                    if let Err(e) = resolve_action_targets(world, action_cid) {
                        eprintln!(
                            "[AnimationSystem] OnAttach resolve failed for {action_cid:?}: {e}"
                        );
                    }
                }
                runtime.attach_resolved = true;
            }

            // Compute beat range for this animation.
            let Some((min_beat, max_beat)) = runtime
                .keyframes
                .iter()
                .filter_map(|&kf_id| {
                    world
                        .get_component_by_id_as::<KeyframeComponent>(kf_id)
                        .map(|kf| kf.beat)
                })
                .fold(None, |acc: Option<(f64, f64)>, beat| match acc {
                    None => Some((beat, beat)),
                    Some((min_b, max_b)) => Some((min_b.min(beat), max_b.max(beat))),
                })
            else {
                continue;
            };

            // Use per-animation local beat time so animations can restart/loop.
            let mut local_beat = (beat_now - runtime.start_beat).max(0.0);
            let span = (max_beat - min_beat).max(0.0);
            // Explicit `Animation.length(n)` wins. Otherwise default:
            // snap to the next whole beat after the last keyframe so
            // common musical loops stay stable even with off-beat
            // keyframes (e.g. max_beat=31.5 → 32.0, not 32.5).
            let loop_len = match length_override {
                Some(n) if n.is_finite() && n > 0.0 => n,
                _ if span < 1e-6 => 1.0,
                _ => span.floor() + 1.0,
            };

            if state == AnimationState::Looping {
                // Wrap local beat into [0, loop_len).
                // When we wrap, clear fired set so keyframes can fire again.
                if local_beat + 1e-9 >= loop_len {
                    let wraps = (local_beat / loop_len).floor();
                    if wraps >= 1.0 {
                        local_beat -= wraps * loop_len;
                        runtime.start_beat = beat_now - local_beat;
                        runtime.fired_keyframes.clear();

                        // Audio scheduling de-dupe is tracked by loop cycle index, so we do
                        // NOT clear it on wrap (lookahead may already have scheduled keyframes
                        // for the next cycle). We just advance the cycle counter.
                        runtime.audio_cycle = runtime.audio_cycle.saturating_add(wraps as u64);
                    }
                }
            }

            // Audio lookahead scheduling phase.
            //
            // Key detail: scheduled audio actions take a beat *offset* relative to the
            // beat context passed into ActionSystem::execute. For lookahead, we want that
            // context to be the keyframe's intended beat time (global), not "now".
            let audio_due = self.evaluator.audio_due_keyframes(
                world,
                anim,
                &runtime.keyframes,
                &runtime.audio_scheduled_cycle_by_keyframe,
                runtime.audio_cycle,
                min_beat,
                local_beat,
                bpm,
                loop_len,
            );

            if !audio_due.is_empty() {
                for (kf_id, kf_local_beat, kf_cycle) in audio_due {
                    let cycle_offset = kf_cycle.saturating_sub(runtime.audio_cycle) as f64;
                    let kf_global_beat =
                        runtime.start_beat + cycle_offset * loop_len + kf_local_beat;

                    let action_ids: Vec<ComponentId> = world
                        .children_of(kf_id)
                        .iter()
                        .copied()
                        .filter(|&cid| {
                            world
                                .get_component_by_id_as::<ActionComponent>(cid)
                                .is_some()
                        })
                        .collect();

                    for action_cid in action_ids {
                        if let Err(e) = resolve_action_targets(world, action_cid) {
                            eprintln!(
                                "[AnimationSystem] lazy resolve failed for {action_cid:?}: {e}"
                            );
                            continue;
                        }
                        let Some(action_comp) =
                            world.get_component_by_id_as::<ActionComponent>(action_cid)
                        else {
                            continue;
                        };

                        let mut signal = action_comp.signal.clone();
                        match &mut signal {
                            IntentValue::OscillatorScheduleSetPitch { beat_context, .. }
                            | IntentValue::AudioSchedulePlay { beat_context, .. } => {
                                // For lookahead, use the keyframe's intended global beat as
                                // the scheduling context (so beat_offset is relative to kf beat).
                                *beat_context = Some(kf_global_beat);
                                rx.push_intent_now(action_cid, signal);
                            }
                            _ => {
                                // Non-audio-scheduled actions must not run in lookahead
                                // (they have immediate side effects).
                            }
                        };
                    }

                    // MusicNote children of the keyframe also schedule via
                    // AudioSchedulePlay — same lookahead semantics as Actions.
                    fire_music_note_children(world, rx, kf_id, Some(kf_global_beat));

                    runtime
                        .audio_scheduled_cycle_by_keyframe
                        .insert(kf_id, kf_cycle);
                }
            }

            let due_keyframes = self.evaluator.visual_due_keyframes(
                world,
                &runtime.keyframes,
                &runtime.fired_keyframes,
                min_beat,
                local_beat,
            );

            for kf_id in due_keyframes {
                let Some(kf) = world.get_component_by_id_as::<KeyframeComponent>(kf_id) else {
                    continue;
                };
                let callback = kf.callback.clone();

                let kf_beat = kf.beat;
                let kf_local_beat = kf_beat - min_beat;

                if kf_local_beat <= local_beat + 1e-9 {
                    if let Some(callback) = callback {
                        if let Err(error) = eval_runtime_closure(
                            &callback,
                            None,
                            Some(world),
                            Some(rx),
                            Some(kf_id),
                            RuntimeClosureExecMode::Full,
                        )
                        {
                            eprintln!(
                                "[AnimationSystem] keyframe callback failed for {kf_id:?}: {error}"
                            );
                        }
                    }

                    // println!(
                    //     "[AnimationSystem] beat {:.3}: keyframe active (kf={:?})",
                    //     kf_beat, kf_id
                    // );

                    let action_ids: Vec<ComponentId> = world
                        .children_of(kf_id)
                        .iter()
                        .copied()
                        .filter(|&cid| {
                            world
                                .get_component_by_id_as::<ActionComponent>(cid)
                                .is_some()
                        })
                        .collect();

                    let mut saw_any_action = false;
                    for action_cid in action_ids {
                        if let Err(e) = resolve_action_targets(world, action_cid) {
                            eprintln!(
                                "[AnimationSystem] lazy resolve failed for {action_cid:?}: {e}"
                            );
                            continue;
                        }
                        let Some(action_comp) =
                            world.get_component_by_id_as::<ActionComponent>(action_cid)
                        else {
                            continue;
                        };

                        saw_any_action = true;

                        // If audio scheduling already happened in lookahead for *this* cycle,
                        // don't re-schedule.
                        if runtime
                            .audio_scheduled_cycle_by_keyframe
                            .get(&kf_id)
                            .copied()
                            == Some(runtime.audio_cycle)
                        {
                            match action_comp.signal {
                                IntentValue::OscillatorScheduleSetPitch { .. }
                                | IntentValue::AudioSchedulePlay { .. } => continue,
                                _ => {}
                            };
                        }

                        let mut signal = action_comp.signal.clone();
                        match &mut signal {
                            IntentValue::OscillatorScheduleSetPitch { beat_context, .. }
                            | IntentValue::AudioSchedulePlay { beat_context, .. } => {
                                // Real-time execution uses the current beat as context.
                                *beat_context = Some(beat_now);
                            }
                            _ => {}
                        };

                        rx.push_intent_now(action_cid, signal);
                    }

                    // Realtime path for MusicNote children — skipped if
                    // lookahead already scheduled this cycle.
                    let already_scheduled = runtime
                        .audio_scheduled_cycle_by_keyframe
                        .get(&kf_id)
                        .copied()
                        == Some(runtime.audio_cycle);
                    if !already_scheduled {
                        fire_music_note_children(world, rx, kf_id, Some(beat_now));
                    }

                    if !saw_any_action {
                        println!("[AnimationSystem] beat {:.3}: (no actions)", kf_beat);
                    }

                    runtime.fired_keyframes.insert(kf_id);
                }
            }

            // Completion: a one-shot animation becomes paused once it has passed its end.
            if state == AnimationState::Playing {
                let done = local_beat + 1e-9 >= loop_len;
                if done {
                    if let Some(anim_comp) =
                        world.get_component_by_id_as_mut::<AnimationComponent>(anim)
                    {
                        anim_comp.state = AnimationState::Paused;
                    }
                }
            }
        }

        self.last_beat = beat_now;
    }
}

impl System for AnimationSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // Driven via `tick_with_beat` from SystemWorld.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{
        AudioOscillatorComponent, ComponentRef, TransformComponent,
    };
    use crate::meow_meow::ast::{
        BinOpKind, BlockStatement, CallExpression, Expression, Ident, Statement,
    };
    use crate::meow_meow::object::{RuntimeClosure, Value};
    use slotmap::Key;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn resolve_action_targets_supports_relative_parent_prefixes() {
        let mut world = World::default();
        let root = world.add_component(TransformComponent::new());
        let target = world.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
        world.add_child(root, target).unwrap();

        let keyframe = world.add_component(KeyframeComponent::new(0.0));
        world.add_child(root, keyframe).unwrap();

        let action = world.add_component(ActionComponent::new_authored(
            IntentValue::SetPosition {
                component_ids: vec![ComponentId::null()],
                position: [1.0, 2.0, 3.0],
            },
            vec![ComponentRef::Query("../../#hero".to_string())],
        ));
        world.add_child(keyframe, action).unwrap();

        resolve_action_targets(&mut world, action).expect("action resolves");

        let action = world
            .get_component_by_id_as::<ActionComponent>(action)
            .expect("action");
        match &action.signal {
            IntentValue::SetPosition { component_ids, .. } => {
                assert_eq!(component_ids, &vec![target]);
            }
            other => panic!("unexpected signal: {other:?}"),
        }
        assert!(action.resolved);
    }

    #[test]
    fn resolve_action_targets_uses_local_scope_for_bare_queries() {
        let mut world = World::default();

        let unrelated_root = world.add_component(TransformComponent::new());
        let unrelated_target =
            world.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
        world.add_child(unrelated_root, unrelated_target).unwrap();

        let local_root = world.add_component(TransformComponent::new());
        let keyframe = world.add_component(KeyframeComponent::new(0.0));
        world.add_child(local_root, keyframe).unwrap();

        let action = world.add_component(ActionComponent::new_authored(
            IntentValue::SetPosition {
                component_ids: vec![ComponentId::null()],
                position: [1.0, 2.0, 3.0],
            },
            vec![ComponentRef::Query("#hero".to_string())],
        ));
        world.add_child(keyframe, action).unwrap();
        let local_target =
            world.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
        world.add_child(action, local_target).unwrap();

        resolve_action_targets(&mut world, action).expect("action resolves");

        let action = world
            .get_component_by_id_as::<ActionComponent>(action)
            .expect("action");
        match &action.signal {
            IntentValue::SetPosition { component_ids, .. } => {
                assert_eq!(component_ids, &vec![local_target]);
                assert_ne!(component_ids, &vec![unrelated_target]);
            }
            other => panic!("unexpected signal: {other:?}"),
        }
    }

    #[test]
    fn resolve_action_targets_use_animation_scope_for_bare_queries() {
        let mut world = World::default();

        let unrelated_root = world.add_component(TransformComponent::new());
        let unrelated_target =
            world.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
        world.add_child(unrelated_root, unrelated_target).unwrap();

        let host = world.add_component(TransformComponent::new());
        let scoped_root =
            world.add_component_boxed_named("avatar_root", Box::new(TransformComponent::new()));
        world.add_child(host, scoped_root).unwrap();
        let scoped_target =
            world.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
        world.add_child(scoped_root, scoped_target).unwrap();

        let animation = world.add_component(
            AnimationComponent::new()
                .with_scope_source(ComponentRef::Query("../#avatar_root".to_string())),
        );
        world.add_child(host, animation).unwrap();

        let keyframe = world.add_component(KeyframeComponent::new(0.0));
        world.add_child(animation, keyframe).unwrap();

        let action = world.add_component(ActionComponent::new_authored(
            IntentValue::SetPosition {
                component_ids: vec![ComponentId::null()],
                position: [1.0, 2.0, 3.0],
            },
            vec![ComponentRef::Query("#hero".to_string())],
        ));
        world.add_child(keyframe, action).unwrap();

        resolve_action_targets(&mut world, action).expect("action resolves");

        let action = world
            .get_component_by_id_as::<ActionComponent>(action)
            .expect("action");
        match &action.signal {
            IntentValue::SetPosition { component_ids, .. } => {
                assert_eq!(component_ids, &vec![scoped_target]);
                assert_ne!(component_ids, &vec![unrelated_target]);
            }
            other => panic!("unexpected signal: {other:?}"),
        }
    }

    #[test]
    fn keyframe_callback_dispatches_live_component_intent_when_due() {
        let mut world = World::default();
        let animation =
            world.add_component(AnimationComponent::new().with_state(AnimationState::Playing));
        let target = world.add_component(TransformComponent::new());
        let callback = RuntimeClosure {
            body: BlockStatement {
                statements: vec![Statement::Expression(Expression::Call(CallExpression {
                    callee: Box::new(Expression::BinaryOp {
                        op: BinOpKind::Dot,
                        lhs: Box::new(Expression::Identifier(Ident("cube_t".to_string()))),
                        rhs: Box::new(Expression::Identifier(Ident(
                            "update_transform".to_string(),
                        ))),
                    }),
                    args: vec![
                        Expression::Array(vec![
                            Expression::Number(1.0),
                            Expression::Number(2.0),
                            Expression::Number(3.0),
                        ]),
                        Expression::Array(vec![
                            Expression::Number(0.0),
                            Expression::Number(0.5),
                            Expression::Number(0.0),
                        ]),
                        Expression::Array(vec![
                            Expression::Number(2.0),
                            Expression::Number(2.0),
                            Expression::Number(2.0),
                        ]),
                    ],
                }))],
            },
            captured_env: Arc::new(HashMap::from([(
                "cube_t".to_string(),
                Value::ComponentObject {
                    id: target,
                    component_type: "Transform".to_string(),
                },
            )])),
            analysis: None,
        };
        let keyframe = world.add_component(KeyframeComponent::new_with_callback(0.0, callback));
        world.add_child(animation, keyframe).unwrap();

        let mut system = AnimationSystem::new();
        system.register_animation(&mut world, animation);
        system.register_keyframe(&mut world, keyframe);

        let mut rx = RxWorld::default();
        system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);

        let intents = rx.drain_ready_intents();
        assert!(intents.iter().any(|signal| {
            matches!(
                signal.intent.as_ref().map(|intent| &intent.value),
                Some(IntentValue::UpdateTransform {
                    component_ids,
                    translation,
                    scale,
                    ..
                }) if component_ids == &vec![target]
                    && *translation == [1.0, 2.0, 3.0]
                    && *scale == [2.0, 2.0, 2.0]
            )
        }));

        let transform = world
            .get_component_by_id_as::<TransformComponent>(target)
            .expect("target transform exists");
        assert_eq!(transform.transform.translation, [1.0, 2.0, 3.0]);
        assert_eq!(transform.transform.scale, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn keyframe_callback_emissive_set_intensity_emits_intensity_intent() {
        let mut world = World::default();
        let animation =
            world.add_component(AnimationComponent::new().with_state(AnimationState::Playing));
        let target = world.add_component(crate::engine::ecs::component::EmissiveComponent::off());
        let callback = RuntimeClosure {
            body: BlockStatement {
                statements: vec![Statement::Expression(Expression::Call(CallExpression {
                    callee: Box::new(Expression::BinaryOp {
                        op: BinOpKind::Dot,
                        lhs: Box::new(Expression::Identifier(Ident("glow".to_string()))),
                        rhs: Box::new(Expression::Identifier(Ident("set_intensity".to_string()))),
                    }),
                    args: vec![Expression::Number(2.5)],
                }))],
            },
            captured_env: Arc::new(HashMap::from([(
                "glow".to_string(),
                Value::ComponentObject {
                    id: target,
                    component_type: "EM".to_string(),
                },
            )])),
            analysis: None,
        };
        let keyframe = world.add_component(KeyframeComponent::new_with_callback(0.0, callback));
        world.add_child(animation, keyframe).unwrap();

        let mut system = AnimationSystem::new();
        system.register_animation(&mut world, animation);
        system.register_keyframe(&mut world, keyframe);

        let mut rx = RxWorld::default();
        system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);

        let intents = rx.drain_ready_intents();
        assert!(intents.iter().any(|signal| {
            matches!(
                signal.intent.as_ref().map(|intent| &intent.value),
                Some(IntentValue::SetEmissiveIntensity {
                    component_ids,
                    intensity,
                }) if component_ids == &vec![target] && (*intensity - 2.5).abs() < 1.0e-6
            )
        }));

        let emissive = world
            .get_component_by_id_as::<crate::engine::ecs::component::EmissiveComponent>(target)
            .expect("target emissive exists");
        assert!((emissive.intensity - 2.5).abs() < 1.0e-6);
    }

    #[test]
    fn runtime_closure_audio_only_filters_visual_and_rewrites_beat_context() {
        let mut world = World::default();
        let glow = world.add_component(crate::engine::ecs::component::EmissiveComponent::off());
        let lead = world.add_component(AudioOscillatorComponent::default());

        let callback = RuntimeClosure {
            body: BlockStatement {
                statements: vec![
                    Statement::Expression(Expression::Call(CallExpression {
                        callee: Box::new(Expression::BinaryOp {
                            op: BinOpKind::Dot,
                            lhs: Box::new(Expression::Identifier(Ident("glow".to_string()))),
                            rhs: Box::new(Expression::Identifier(Ident(
                                "set_intensity".to_string(),
                            ))),
                        }),
                        args: vec![Expression::Number(2.5)],
                    })),
                    Statement::Expression(Expression::Call(CallExpression {
                        callee: Box::new(Expression::BinaryOp {
                            op: BinOpKind::Dot,
                            lhs: Box::new(Expression::Identifier(Ident("MusicNote".to_string()))),
                            rhs: Box::new(Expression::Identifier(Ident("e".to_string()))),
                        }),
                        args: vec![
                            Expression::Number(4.0),
                            Expression::Number(0.25),
                            Expression::Identifier(Ident("lead".to_string())),
                        ],
                    })),
                ],
            },
            captured_env: Arc::new(HashMap::from([
                (
                    "glow".to_string(),
                    Value::ComponentObject {
                        id: glow,
                        component_type: "EM".to_string(),
                    },
                ),
                (
                    "lead".to_string(),
                    Value::ComponentObject {
                        id: lead,
                        component_type: "AudioOscillator".to_string(),
                    },
                ),
            ])),
            analysis: None,
        };

        let mut rx = RxWorld::default();
        eval_runtime_closure(
            &callback,
            None,
            Some(&mut world),
            Some(&mut rx),
            None,
            RuntimeClosureExecMode::KeyframeAudioOnly { beat_context: 12.5 },
        )
        .expect("audio-only runtime closure eval succeeds");

        let intents = rx.drain_ready_intents();
        assert_eq!(intents.len(), 1);
        assert!(intents.iter().any(|signal| {
            matches!(
                signal.intent.as_ref().map(|intent| &intent.value),
                Some(IntentValue::AudioSchedulePlay {
                    component_ids,
                    beat_context,
                    ..
                }) if component_ids == &vec![lead] && *beat_context == Some(12.5)
            )
        }));
    }

    #[test]
    fn runtime_closure_visual_only_filters_audio() {
        let mut world = World::default();
        let glow = world.add_component(crate::engine::ecs::component::EmissiveComponent::off());
        let lead = world.add_component(AudioOscillatorComponent::default());

        let callback = RuntimeClosure {
            body: BlockStatement {
                statements: vec![
                    Statement::Expression(Expression::Call(CallExpression {
                        callee: Box::new(Expression::BinaryOp {
                            op: BinOpKind::Dot,
                            lhs: Box::new(Expression::Identifier(Ident("MusicNote".to_string()))),
                            rhs: Box::new(Expression::Identifier(Ident("e".to_string()))),
                        }),
                        args: vec![
                            Expression::Number(4.0),
                            Expression::Number(0.25),
                            Expression::Identifier(Ident("lead".to_string())),
                        ],
                    })),
                    Statement::Expression(Expression::Call(CallExpression {
                        callee: Box::new(Expression::BinaryOp {
                            op: BinOpKind::Dot,
                            lhs: Box::new(Expression::Identifier(Ident("glow".to_string()))),
                            rhs: Box::new(Expression::Identifier(Ident(
                                "set_intensity".to_string(),
                            ))),
                        }),
                        args: vec![Expression::Number(2.5)],
                    })),
                ],
            },
            captured_env: Arc::new(HashMap::from([
                (
                    "glow".to_string(),
                    Value::ComponentObject {
                        id: glow,
                        component_type: "EM".to_string(),
                    },
                ),
                (
                    "lead".to_string(),
                    Value::ComponentObject {
                        id: lead,
                        component_type: "AudioOscillator".to_string(),
                    },
                ),
            ])),
            analysis: None,
        };

        let mut rx = RxWorld::default();
        eval_runtime_closure(
            &callback,
            None,
            Some(&mut world),
            Some(&mut rx),
            None,
            RuntimeClosureExecMode::KeyframeVisualOnly,
        )
        .expect("visual-only runtime closure eval succeeds");

        let intents = rx.drain_ready_intents();
        assert_eq!(intents.len(), 1);
        assert!(intents.iter().any(|signal| {
            matches!(
                signal.intent.as_ref().map(|intent| &intent.value),
                Some(IntentValue::SetEmissiveIntensity {
                    component_ids,
                    intensity,
                }) if component_ids == &vec![glow] && (*intensity - 2.5).abs() < 1.0e-6
            )
        }));
    }
}
