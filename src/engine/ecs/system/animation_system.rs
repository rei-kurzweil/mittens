use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::engine::ecs::component::{
    AnimationComponent, AnimationState, AnimationStepDirection, KeyframeComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::animation_keyframe_evaluator::AnimationKeyframeEvaluator;
use crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor;
use crate::engine::ecs::system::animation_scheduler::AnimationScheduler;
use crate::engine::ecs::{ComponentId, RxWorld, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
struct AnimationRuntime {
    keyframes: Vec<ComponentId>,
    manual_cursor: Option<ComponentId>,
    manual_cursor_ordinal: Option<usize>,
    fired_keyframes: BTreeSet<ComponentId>,
    /// For audio lookahead scheduling, track the last loop-cycle index each keyframe was
    /// scheduled for.
    audio_scheduled_cycle_by_keyframe: BTreeMap<ComponentId, u64>,
    /// Loop cycle index for audio scheduling. Increments whenever a looping animation wraps.
    audio_cycle: u64,
    start_beat: f64,
    pending_commands: VecDeque<AnimationCommand>,
}

#[derive(Debug, Clone, Copy)]
enum AnimationCommand {
    SetState(AnimationState),
    Step(AnimationStepDirection),
}

#[derive(Debug, Default)]
pub struct AnimationSystem {
    /// Runtime state keyed by `AnimationComponent` id.
    ///
    /// BTree* gives deterministic iteration order (nice for debugging/logs).
    animations: BTreeMap<ComponentId, AnimationRuntime>,
    last_beat: f64,

    scheduler: AnimationScheduler,
    keyframe_evaluator: AnimationKeyframeEvaluator,
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
            .pending_commands
            .push_back(AnimationCommand::SetState(state));
    }

    pub fn step_animation(&mut self, animation: ComponentId, direction: AnimationStepDirection) {
        self.animations
            .entry(animation)
            .or_insert_with(AnimationRuntime::default)
            .pending_commands
            .push_back(AnimationCommand::Step(direction));
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
                if let Some(cursor) = runtime.manual_cursor {
                    runtime.manual_cursor_ordinal = list.iter().position(|id| *id == cursor);
                }
                return;
            }
            cursor = world.parent_of(node);
        }
    }

    pub fn tick_with_beat(&mut self, world: &mut World, beat_now: f64, bpm: f64, rx: &mut RxWorld) {
        self.tick_with_beat_and_executor(world, beat_now, bpm, rx, None);
    }

    pub(crate) fn tick_with_beat_and_executor(
        &mut self,
        world: &mut World,
        beat_now: f64,
        bpm: f64,
        rx: &mut RxWorld,
        mut executor: Option<&mut SessionCallbackExecutor<'_>>,
    ) {
        // If time jumps backwards, reset fired state.
        if beat_now + 1e-9 < self.last_beat {
            for runtime in self.animations.values_mut() {
                runtime.fired_keyframes.clear();
                runtime.audio_scheduled_cycle_by_keyframe.clear();
                runtime.audio_cycle = 0;
            }
        }

        // Remove stale keyframes before resolving cursor-relative commands. If the selected
        // keyframe disappeared, retain the nearest surviving ordinal where possible.
        for runtime in self.animations.values_mut() {
            let prior_ordinal = runtime.manual_cursor_ordinal;
            runtime.keyframes.retain(|keyframe| {
                world
                    .get_component_by_id_as::<KeyframeComponent>(*keyframe)
                    .is_some()
            });
            if runtime
                .manual_cursor
                .is_some_and(|cursor| !runtime.keyframes.contains(&cursor))
            {
                if let Some(index) = prior_ordinal.filter(|_| !runtime.keyframes.is_empty()) {
                    let index = index.min(runtime.keyframes.len() - 1);
                    runtime.manual_cursor = Some(runtime.keyframes[index]);
                    runtime.manual_cursor_ordinal = Some(index);
                } else {
                    runtime.manual_cursor = None;
                    runtime.manual_cursor_ordinal = None;
                }
            }
        }

        // Apply state changes and manual steps in the order their intents arrived.
        for (&anim, runtime) in self.animations.iter_mut() {
            while let Some(command) = runtime.pending_commands.pop_front() {
                match command {
                    AnimationCommand::SetState(state) => {
                        let Some(anim_comp) =
                            world.get_component_by_id_as_mut::<AnimationComponent>(anim)
                        else {
                            break;
                        };

                        anim_comp.state = state;
                        runtime.start_beat = beat_now;
                        runtime.fired_keyframes.clear();
                        runtime.audio_scheduled_cycle_by_keyframe.clear();
                        runtime.audio_cycle = 0;
                        if matches!(state, AnimationState::Playing | AnimationState::Looping) {
                            runtime.manual_cursor = None;
                            runtime.manual_cursor_ordinal = None;
                        }
                    }
                    AnimationCommand::Step(direction) => {
                        let Some(anim_comp) =
                            world.get_component_by_id_as_mut::<AnimationComponent>(anim)
                        else {
                            break;
                        };
                        anim_comp.state = AnimationState::Paused;

                        let selected_index = match direction {
                            AnimationStepDirection::Next => match runtime.manual_cursor_ordinal {
                                Some(index) => index.checked_add(1),
                                None => Some(0),
                            },
                            AnimationStepDirection::Previous => runtime
                                .manual_cursor_ordinal
                                .and_then(|index| index.checked_sub(1)),
                        }
                        .filter(|index| *index < runtime.keyframes.len());

                        let Some(selected_index) = selected_index else {
                            continue;
                        };
                        let keyframe = runtime.keyframes[selected_index];
                        runtime.manual_cursor = Some(keyframe);
                        runtime.manual_cursor_ordinal = Some(selected_index);

                        // Manual stepping is intentionally visual-only. Passing `true` here
                        // suppresses MusicNote child playback as well as closure audio intents.
                        self.keyframe_evaluator.evaluate_visual_due_keyframe(
                            world,
                            rx,
                            keyframe,
                            beat_now,
                            true,
                            executor.as_deref_mut(),
                        );
                    }
                }
            }
        }

        // Drive animations.
        for (&anim, runtime) in self.animations.iter_mut() {
            let (state, length_override) =
                match world.get_component_by_id_as::<AnimationComponent>(anim) {
                    Some(c) => (c.state, c.length_beats),
                    None => continue,
                };

            if state == AnimationState::Paused {
                continue;
            }

            if runtime.keyframes.is_empty() {
                continue;
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
            // beat context passed into keyframe evaluation. For lookahead, we want that
            // context to be the keyframe's intended beat time (global), not "now".
            let audio_due = self.scheduler.audio_due_keyframes(
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

                    self.keyframe_evaluator.evaluate_audio_due_keyframe(
                        world,
                        rx,
                        kf_id,
                        kf_global_beat,
                        executor.as_deref_mut(),
                    );

                    runtime
                        .audio_scheduled_cycle_by_keyframe
                        .insert(kf_id, kf_cycle);
                }
            }

            let due_keyframes = self.scheduler.visual_due_keyframes(
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
                let kf_local_beat = kf.beat - min_beat;

                if kf_local_beat <= local_beat + 1e-9 {
                    let already_scheduled = runtime
                        .audio_scheduled_cycle_by_keyframe
                        .get(&kf_id)
                        .copied()
                        == Some(runtime.audio_cycle);
                    self.keyframe_evaluator.evaluate_visual_due_keyframe(
                        world,
                        rx,
                        kf_id,
                        beat_now,
                        already_scheduled,
                        executor.as_deref_mut(),
                    );

                    runtime.fired_keyframes.insert(kf_id);
                    runtime.manual_cursor = Some(kf_id);
                    runtime.manual_cursor_ordinal =
                        runtime.keyframes.iter().position(|id| *id == kf_id);
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

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{AudioOscillatorComponent, TransformComponent};
    use crate::scripting::ast::{
        BinOpKind, BlockStatement, CallExpression, Expression, Ident, Statement,
    };
    use crate::scripting::object::{RuntimeClosure, Value};
    use crate::scripting::world_evaluator::{RuntimeClosureExecMode, eval_runtime_closure};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn emissive_intensity_callback(target: ComponentId, intensity: f64) -> RuntimeClosure {
        RuntimeClosure {
            body: BlockStatement {
                statements: vec![Statement::Expression(Expression::Call(CallExpression {
                    callee: Box::new(Expression::BinaryOp {
                        op: BinOpKind::Dot,
                        lhs: Box::new(Expression::Identifier(Ident("glow".to_string()))),
                        rhs: Box::new(Expression::Identifier(Ident("set_intensity".to_string()))),
                    }),
                    args: vec![Expression::Number(intensity)],
                }))],
            },
            captured_env: Arc::new(HashMap::from([(
                "glow".to_string(),
                Value::ComponentObject {
                    id: target,
                    component_type: "EM".to_string(),
                },
            )])),
            heap: crate::scripting::object::HeapHandle::new(),
            analysis: None,
        }
    }

    fn stepped_intensities(rx: &mut RxWorld) -> Vec<f32> {
        rx.drain_ready_intents()
            .into_iter()
            .filter_map(|signal| match signal.intent.map(|intent| intent.value) {
                Some(IntentValue::SetEmissiveIntensity { intensity, .. }) => Some(intensity),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn paused_animation_steps_forward_and_backward_with_clamped_ends() {
        let mut world = World::default();
        let animation =
            world.add_component(AnimationComponent::new().with_state(AnimationState::Paused));
        let target = world.add_component(crate::engine::ecs::component::EmissiveComponent::off());
        let keyframes = [(0.0, 1.0), (10.0, 2.0), (100.0, 3.0)].map(|(beat, intensity)| {
            let keyframe = world.add_component(KeyframeComponent::new_with_callback(
                beat,
                emissive_intensity_callback(target, intensity),
            ));
            world.add_child(animation, keyframe).unwrap();
            keyframe
        });

        let mut system = AnimationSystem::new();
        system.register_animation(&mut world, animation);
        for keyframe in keyframes {
            system.register_keyframe(&mut world, keyframe);
        }
        let mut rx = RxWorld::default();

        for expected in [1.0, 2.0, 3.0] {
            system.step_animation(animation, AnimationStepDirection::Next);
            system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);
            assert_eq!(stepped_intensities(&mut rx), vec![expected]);
        }
        system.step_animation(animation, AnimationStepDirection::Next);
        system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);
        assert!(stepped_intensities(&mut rx).is_empty());

        for expected in [2.0, 1.0] {
            system.step_animation(animation, AnimationStepDirection::Previous);
            system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);
            assert_eq!(stepped_intensities(&mut rx), vec![expected]);
        }
        system.step_animation(animation, AnimationStepDirection::Previous);
        system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);
        assert!(stepped_intensities(&mut rx).is_empty());
        assert_eq!(
            world
                .get_component_by_id_as::<AnimationComponent>(animation)
                .unwrap()
                .state,
            AnimationState::Paused
        );
    }

    #[test]
    fn manual_step_pauses_playback_and_advances_from_last_visual_keyframe() {
        let mut world = World::default();
        let animation =
            world.add_component(AnimationComponent::new().with_state(AnimationState::Playing));
        let target = world.add_component(crate::engine::ecs::component::EmissiveComponent::off());
        for (beat, intensity) in [(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)] {
            let keyframe = world.add_component(KeyframeComponent::new_with_callback(
                beat,
                emissive_intensity_callback(target, intensity),
            ));
            world.add_child(animation, keyframe).unwrap();
        }

        let keyframes = world.children_of(animation).to_vec();
        let mut system = AnimationSystem::new();
        system.register_animation(&mut world, animation);
        for keyframe in keyframes {
            system.register_keyframe(&mut world, keyframe);
        }
        let mut rx = RxWorld::default();

        system.tick_with_beat(&mut world, 1.0, 60.0, &mut rx);
        assert_eq!(stepped_intensities(&mut rx), vec![1.0, 2.0]);

        system.step_animation(animation, AnimationStepDirection::Next);
        system.tick_with_beat(&mut world, 1.0, 60.0, &mut rx);
        assert_eq!(stepped_intensities(&mut rx), vec![3.0]);
        assert_eq!(
            world
                .get_component_by_id_as::<AnimationComponent>(animation)
                .unwrap()
                .state,
            AnimationState::Paused
        );
    }

    #[test]
    fn restarting_timed_playback_resets_manual_cursor_and_preserves_default_looping() {
        let mut world = World::default();
        let animation = world.add_component(AnimationComponent::new());
        assert_eq!(
            world
                .get_component_by_id_as::<AnimationComponent>(animation)
                .unwrap()
                .state,
            AnimationState::Looping
        );
        let target = world.add_component(crate::engine::ecs::component::EmissiveComponent::off());
        for (beat, intensity) in [(0.0, 1.0), (1.0, 2.0)] {
            let keyframe = world.add_component(KeyframeComponent::new_with_callback(
                beat,
                emissive_intensity_callback(target, intensity),
            ));
            world.add_child(animation, keyframe).unwrap();
        }

        let keyframes = world.children_of(animation).to_vec();
        let mut system = AnimationSystem::new();
        system.register_animation(&mut world, animation);
        for keyframe in keyframes {
            system.register_keyframe(&mut world, keyframe);
        }
        let mut rx = RxWorld::default();

        system.step_animation(animation, AnimationStepDirection::Next);
        system.tick_with_beat(&mut world, 0.0, 60.0, &mut rx);
        assert_eq!(stepped_intensities(&mut rx), vec![1.0]);

        system.set_animation_state(animation, AnimationState::Looping);
        system.tick_with_beat(&mut world, 5.0, 60.0, &mut rx);
        assert_eq!(stepped_intensities(&mut rx), vec![1.0]);

        system.step_animation(animation, AnimationStepDirection::Next);
        system.tick_with_beat(&mut world, 5.0, 60.0, &mut rx);
        assert_eq!(stepped_intensities(&mut rx), vec![2.0]);
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
            heap: crate::scripting::object::HeapHandle::new(),
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
                    component_id,
                    translation,
                    scale,
                    ..
                }) if component_id == &target
                    && *translation == [1.0, 2.0, 3.0]
                    && *scale == [2.0, 2.0, 2.0]
            )
        }));
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
            heap: crate::scripting::object::HeapHandle::new(),
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
                    component_id,
                    intensity,
                }) if component_id == &target && (*intensity - 2.5).abs() < 1.0e-6
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
            heap: crate::scripting::object::HeapHandle::new(),
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
                    component_id,
                    beat_context,
                    ..
                }) if component_id == &lead && *beat_context == Some(12.5)
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
            heap: crate::scripting::object::HeapHandle::new(),
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
                    component_id,
                    intensity,
                }) if component_id == &glow && (*intensity - 2.5).abs() < 1.0e-6
            )
        }));
    }
}
