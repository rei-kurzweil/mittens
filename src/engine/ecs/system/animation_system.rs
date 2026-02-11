use std::collections::{BTreeMap, BTreeSet};

use crate::engine::ecs::component::{
    ActionComponent, ActionMethod, AnimationComponent, AnimationState, KeyframeComponent,
};
use crate::engine::ecs::system::ActionSystem;
use crate::engine::ecs::system::animation_system_evaluator::AnimationEvaluator;
use crate::engine::ecs::system::System;
use crate::engine::ecs::{CommandQueue, ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

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
            if world.get_component_by_id_as::<AnimationComponent>(node).is_some() {
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

    pub fn tick_with_beat(
        &mut self,
        world: &mut World,
        beat_now: f64,
        bpm: f64,
        action_system: &mut ActionSystem,
        queue: &mut CommandQueue,
    ) {
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
            if matches!(state, AnimationState::Playing | AnimationState::Looping) {
                runtime.start_beat = beat_now;
                runtime.fired_keyframes.clear();
                runtime.audio_scheduled_cycle_by_keyframe.clear();
                runtime.audio_cycle = 0;
            }
        }

        // Drive animations.
        for (&anim, runtime) in self.animations.iter_mut() {
            let state = match world.get_component_by_id_as::<AnimationComponent>(anim) {
                Some(c) => c.state,
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
            // Default loop length: snap to the next whole beat after the last keyframe.
            // This keeps common musical loops stable even when you include off-beat keyframes
            // (e.g. max_beat=31.5 should loop at 32.0 beats, not 32.5).
            let loop_len = if span < 1e-6 {
                1.0
            } else {
                span.floor() + 1.0
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
                        runtime.audio_cycle = runtime
                            .audio_cycle
                            .saturating_add(wraps as u64);
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
                    let kf_global_beat = runtime.start_beat + cycle_offset * loop_len + kf_local_beat;

                    let action_ids: Vec<ComponentId> = world
                        .children_of(kf_id)
                        .iter()
                        .copied()
                        .filter(|&cid| world.get_component_by_id_as::<ActionComponent>(cid).is_some())
                        .collect();

                    for action_cid in action_ids {
                        let Some(action_comp) =
                            world.get_component_by_id_as::<ActionComponent>(action_cid)
                        else {
                            continue;
                        };

                        let action = action_comp.action.clone();
                        match action.method {
                            ActionMethod::OscillatorScheduleSetPitch
                            | ActionMethod::OscillatorScheduleSetNote
                            | ActionMethod::OscillatorScheduleMusicNote => {
                                action_system.execute(world, queue, kf_global_beat, &action);
                            }
                            _ => {
                                // Non-audio-scheduled actions must not run in lookahead
                                // (they have immediate side effects).
                            }
                        }
                    }

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

                let kf_beat = kf.beat;
                let kf_local_beat = kf_beat - min_beat;

                if kf_local_beat <= local_beat + 1e-9 {
                    // println!(
                    //     "[AnimationSystem] beat {:.3}: keyframe active (kf={:?})",
                    //     kf_beat, kf_id
                    // );

                    let action_ids: Vec<ComponentId> = world
                        .children_of(kf_id)
                        .iter()
                        .copied()
                        .filter(|&cid| world.get_component_by_id_as::<ActionComponent>(cid).is_some())
                        .collect();

                    let mut saw_any_action = false;
                    for action_cid in action_ids {
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
                            match action_comp.action.method {
                                ActionMethod::OscillatorScheduleSetPitch
                                | ActionMethod::OscillatorScheduleSetNote
                                | ActionMethod::OscillatorScheduleMusicNote => {
                                    continue;
                                }
                                _ => {}
                            }
                        }

                        let action = action_comp.action.clone();
                        action_system.execute(world, queue, beat_now, &action);
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
                    if let Some(anim_comp) = world.get_component_by_id_as_mut::<AnimationComponent>(anim)
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

/*
use std::collections::{BTreeMap, BTreeSet};

use crate::engine::ecs::component::{ActionComponent, AnimationComponent, AnimationState, KeyframeComponent};
use crate::engine::ecs::system::ActionSystem;
use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
struct AnimationRuntime {
    keyframes: Vec<ComponentId>,
    fired_keyframes: BTreeSet<ComponentId>,
    start_beat: f64,
    pending_state: Option<AnimationState>,
}

#[derive(Debug, Default)]
pub struct AnimationSystem {
    animations: BTreeMap<ComponentId, AnimationRuntime>,
    last_beat: f64,
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

        self.animations.entry(component).or_insert_with(AnimationRuntime::default);
    }

    pub fn set_animation_state(&mut self, animation: ComponentId, state: AnimationState) {
        if let Some(runtime) = self.animations.get_mut(&animation) {
            runtime.pending_state = Some(state);
        }
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
            if world.get_component_by_id_as::<AnimationComponent>(node).is_some() {
                self.animations.insert(node);
                let list = self.keyframes_by_animation.entry(node).or_default();
                if !list.contains(&component) {
                    list.push(component);
                }
                self.fired_keyframes_by_animation.entry(node).or_default();
                self.start_beat_by_animation.entry(node).or_insert(0.0);
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

    pub fn tick_with_beat(
        &mut self,
        world: &mut World,
        beat_now: f64,
        action_system: &mut ActionSystem,
        queue: &mut CommandQueue,
                let runtime = self.animations.entry(node).or_insert_with(AnimationRuntime::default);
                let list = &mut runtime.keyframes;
        if beat_now + 1e-9 < self.last_beat {
            for fired in self.fired_keyframes_by_animation.values_mut() {
                fired.clear();

        // Apply any requested state changes.
        // We also treat transitions into Playing/Looping as "restart".
        let pending: Vec<(ComponentId, AnimationState)> =
            self.pending_state_by_animation.iter().map(|(k, v)| (*k, *v)).collect();
        if !pending.is_empty() {
            self.pending_state_by_animation.clear();
            for (anim, state) in pending {
                let Some(anim_comp) = world.get_component_by_id_as_mut::<AnimationComponent>(anim)
                else {
                    continue;
                };

                anim_comp.state = state;
                if matches!(state, AnimationState::Playing | AnimationState::Looping) {
                    self.start_beat_by_animation.insert(anim, beat_now);
                    self.fired_keyframes_by_animation
                        .entry(anim)
                        .or_default()
                        .clear();
                }
            }
        }

        for &anim in self.animations.iter() {
            let state = match world.get_component_by_id_as::<AnimationComponent>(anim) {
                Some(c) => c.state,
            for runtime in self.animations.values_mut() {
                runtime.fired_keyframes.clear();
            }
                continue;
            }

            let Some(keyframes) = self.keyframes_by_animation.get(&anim) else {
                continue;
            self.animations.iter_mut().filter_map(|(&anim, runtime)| {
                let Some(state) = runtime.pending_state.take() else {
                    return None;
                };

                let Some(anim_comp) = world.get_component_by_id_as_mut::<AnimationComponent>(anim) else {
                    return None;
                };

                anim_comp.state = state;
                if matches!(state, AnimationState::Playing | AnimationState::Looping) {
                    runtime.start_beat = beat_now;
                    runtime.fired_keyframes.clear();
                }
                Some(anim)
            }).collect();
        }
            // Default loop length: inclusive of last beat.
            let loop_len = if span < 1e-6 { 1.0 } else { span + 1.0 };

            if state == AnimationState::Looping {
                // Wrap local beat into [0, loop_len).
                // When we wrap, also clear fired set so keyframes can fire again.
                if local_beat + 1e-9 >= loop_len {
                    let wraps = (local_beat / loop_len).floor();
                    if wraps >= 1.0 {
                        local_beat -= wraps * loop_len;
            if runtime.keyframes.is_empty() {
                            .insert(anim, beat_now - local_beat);
                        self.fired_keyframes_by_animation

            let keyframes = &runtime.keyframes;
                            .entry(anim)
                            .or_default()
            let mut local_beat = (beat_now - runtime.start_beat).max(0.0);
                }
            }

            let fired = self.fired_keyframes_by_animation.entry(anim).or_default();

            for &kf_id in keyframes.iter() {
                if fired.contains(&kf_id) {
                    continue;
                }

                let Some(kf) = world.get_component_by_id_as::<KeyframeComponent>(kf_id) else {
                        runtime.start_beat = beat_now - local_beat;
                        runtime.fired_keyframes.clear();
                if kf_local_beat <= local_beat + 1e-9 {
                    println!(
                        "[AnimationSystem] beat {:.3}: keyframe active (kf={:?})",
                        kf_beat, kf_id

                    let action_ids: Vec<ComponentId> = world
                if runtime.fired_keyframes.contains(&kf_id) {
                        .iter()
                        .copied()
                        .filter(|&cid| world.get_component_by_id_as::<ActionComponent>(cid).is_some())
                        .collect();

                    let mut ran_any_action = false;
                    for action_cid in action_ids {
                        let Some(action_comp) =
                            world.get_component_by_id_as::<ActionComponent>(action_cid)
                        else {
                            continue;
                        };

                        let action = action_comp.action.clone();
                        ran_any_action = true;
                        action_system.execute(world, queue, beat_now, &action);
                    }

                    if !ran_any_action {
                        println!("[AnimationSystem] beat {:.3}: (no actions)", kf_beat);
                    }

                    fired.insert(kf_id);
                }
            }

            // Completion: a one-shot animation becomes paused once it has passed its end.
            if state == AnimationState::Playing {
                let done = local_beat + 1e-9 >= loop_len;
                if done {
                    if let Some(anim_comp) = world.get_component_by_id_as_mut::<AnimationComponent>(anim)
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

*/
