use std::collections::{BTreeMap, BTreeSet};

use crate::engine::ecs::component::{
    AnimationComponent, AnimationState, KeyframeComponent, MusicNoteComponent,
};
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
pub(crate) struct AnimationScheduler {
    pub(crate) config: AnimationEvalConfig,
}

impl AnimationScheduler {
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
            if !kf.effect_profile.runs_in_visual_phase() && !has_music_note_child(world, kf_id) {
                continue;
            }
            let kf_local_beat = kf.beat - min_beat;
            if kf_local_beat <= local_beat + 1e-9 {
                out.push(kf_id);
            }
        }
        out
    }

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
        let loop_len = if loop_len.is_finite() {
            loop_len.max(0.0)
        } else {
            0.0
        };

        let mut out = Vec::new();
        for &kf_id in keyframes.iter() {
            let scheduled_cycle = scheduled_cycle_by_keyframe.get(&kf_id).copied();

            let Some(kf) = world.get_component_by_id_as::<KeyframeComponent>(kf_id) else {
                continue;
            };
            if !kf.effect_profile.runs_in_audio_phase() && !has_music_note_child(world, kf_id) {
                continue;
            }
            let kf_local_beat = kf.beat - min_beat;

            if kf_local_beat >= local_beat - 1e-9 && kf_local_beat <= local_end + 1e-9 {
                if scheduled_cycle != Some(current_cycle) {
                    out.push((kf_id, kf_local_beat, current_cycle));
                }
                continue;
            }

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

fn has_music_note_child(world: &World, keyframe: ComponentId) -> bool {
    world.children_of(keyframe).iter().copied().any(|child| {
        world
            .get_component_by_id_as::<MusicNoteComponent>(child)
            .is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{AnimationComponent, KeyframeComponent};
    use meow_meow_script::{
        CallbackHandle, KeyframeEffectProfile, SessionCallbackRef, SessionHandle,
    };

    fn keyframe(world: &mut World, beat: f64, profile: KeyframeEffectProfile) -> ComponentId {
        world.add_component(KeyframeComponent::new_with_session_callback(
            beat,
            SessionCallbackRef {
                session: SessionHandle::from_raw(1),
                callback: CallbackHandle::from_raw(1),
            },
            profile,
        ))
    }

    #[test]
    fn phase_pure_callbacks_are_not_returned_for_the_irrelevant_phase() {
        let mut world = World::default();
        let animation = world.add_component(AnimationComponent::new());
        let audio = keyframe(&mut world, 0.0, KeyframeEffectProfile::AudioOnly);
        let visual = keyframe(&mut world, 0.0, KeyframeEffectProfile::VisualOnly);
        let none = keyframe(&mut world, 0.0, KeyframeEffectProfile::None);
        let unknown = keyframe(&mut world, 0.0, KeyframeEffectProfile::Unknown);
        let keyframes = [audio, visual, none, unknown];
        let scheduler = AnimationScheduler::new().with_lookahead_sec(0.1);

        let audio_due = scheduler.audio_due_keyframes(
            &world,
            animation,
            &keyframes,
            &BTreeMap::new(),
            0,
            0.0,
            0.0,
            60.0,
            1.0,
        );
        assert_eq!(
            audio_due.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
            vec![audio, unknown]
        );

        let visual_due =
            scheduler.visual_due_keyframes(&world, &keyframes, &BTreeSet::new(), 0.0, 0.0);
        assert_eq!(visual_due, vec![visual, unknown]);
    }
}
