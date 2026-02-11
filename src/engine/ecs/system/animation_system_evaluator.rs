use std::collections::{BTreeMap, BTreeSet};

use crate::engine::ecs::component::{AnimationComponent, AnimationState, KeyframeComponent};
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, Copy)]
pub(crate) struct AnimationEvalConfig {
    /// How far ahead to evaluate audio keyframes, in seconds.
    pub(crate) audio_lookahead_sec: f64,
}

impl Default for AnimationEvalConfig {
    fn default() -> Self {
        Self {
            audio_lookahead_sec: 0.100,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct AnimationEvaluator {
    pub(crate) config: AnimationEvalConfig,
}

impl AnimationEvaluator {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_lookahead_sec(mut self, sec: f64) -> Self {
        if sec.is_finite() && sec >= 0.0 {
            self.config.audio_lookahead_sec = sec;
        }
        self
    }

    pub(crate) fn audio_lookahead_beats(&self, bpm: f64) -> f64 {
        if !bpm.is_finite() || bpm <= 0.0 {
            return 0.0;
        }
        let beats_per_sec = bpm / 60.0;
        (self.config.audio_lookahead_sec.max(0.0)) * beats_per_sec
    }

    /// Returns keyframes that should fire for the visual evaluation phase.
    ///
    /// Current semantics: returns all keyframes with `kf_local_beat <= local_beat` that have
    /// not been fired yet.
    pub(crate) fn visual_due_keyframes(
        &self,
        world: &World,
        keyframes: &[ComponentId],
        fired_keyframes: &BTreeSet<ComponentId>,
        min_beat: f64,
        local_beat: f64,
    ) -> Vec<ComponentId> {
        let mut out = Vec::new();
        for &kf_id in keyframes.iter() {
            if fired_keyframes.contains(&kf_id) {
                continue;
            }
            let Some(kf) = world.get_component_by_id_as::<KeyframeComponent>(kf_id) else {
                continue;
            };
            let kf_local_beat = kf.beat - min_beat;
            if kf_local_beat <= local_beat + 1e-9 {
                out.push(kf_id);
            }
        }
        out
    }

    /// Returns keyframes that should be evaluated for the audio lookahead phase.
    ///
    /// NOTE: we intentionally do not use this yet; it is scaffolding for upcoming
    /// sample-precise audio scheduling.
    pub(crate) fn audio_due_keyframes(
        &self,
        world: &World,
        anim_id: ComponentId,
        keyframes: &[ComponentId],
        scheduled_cycle_by_keyframe: &BTreeMap<ComponentId, u64>,
        current_cycle: u64,
        min_beat: f64,
        local_beat: f64,
        bpm: f64,
        loop_len: f64,
    ) -> Vec<(ComponentId, f64, u64)> {
        let Some(anim) = world.get_component_by_id_as::<AnimationComponent>(anim_id) else {
            return Vec::new();
        };
        if anim.state == AnimationState::Paused {
            return Vec::new();
        }

        let lookahead = self.audio_lookahead_beats(bpm);
        if lookahead <= 0.0 {
            return Vec::new();
        }
        let local_end = local_beat + lookahead;

        let is_looping = anim.state == AnimationState::Looping;
        let loop_len = if loop_len.is_finite() { loop_len.max(0.0) } else { 0.0 };

        let mut out = Vec::new();
        for &kf_id in keyframes.iter() {
            let scheduled_cycle = scheduled_cycle_by_keyframe.get(&kf_id).copied();

            let Some(kf) = world.get_component_by_id_as::<KeyframeComponent>(kf_id) else {
                continue;
            };
            let kf_local_beat = kf.beat - min_beat;

            // Segment A: remaining part of current cycle.
            if kf_local_beat > local_beat + 1e-9 && kf_local_beat <= local_end + 1e-9 {
                if scheduled_cycle != Some(current_cycle) {
                    out.push((kf_id, kf_local_beat, current_cycle));
                }
                continue;
            }

            // Segment B: if lookahead crosses the loop boundary, also schedule early keyframes
            // in the *next* cycle.
            if is_looping && loop_len > 1e-9 && local_end > loop_len + 1e-9 {
                let next_end = local_end - loop_len;
                if kf_local_beat >= 0.0 - 1e-9 && kf_local_beat <= next_end + 1e-9 {
                    let next_cycle = current_cycle.saturating_add(1);
                    if scheduled_cycle != Some(next_cycle) {
                        out.push((kf_id, kf_local_beat, next_cycle));
                    }
                }
            }
        }
        out
    }
}
