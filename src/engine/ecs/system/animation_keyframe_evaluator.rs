use crate::engine::ecs::component::{
    ActionComponent, AnimationComponent, KeyframeComponent, MusicNoteComponent, QueryRootMode,
    action::{apply_resolved_targets, signal_target_slot_count},
    resolve_component_ref,
};
use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, World};
use crate::meow_meow::evaluator::{RuntimeClosureExecMode, eval_runtime_closure};

#[derive(Debug, Default)]
pub(crate) struct AnimationKeyframeEvaluator;

impl AnimationKeyframeEvaluator {
    pub(crate) fn resolve_action_targets(
        &self,
        world: &mut World,
        action_id: ComponentId,
    ) -> Result<(), String> {
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

    pub(crate) fn evaluate_audio_due_keyframe(
        &self,
        world: &mut World,
        rx: &mut RxWorld,
        kf_id: ComponentId,
        kf_global_beat: f64,
    ) {
        let runtime_closure = world
            .get_component_by_id_as::<KeyframeComponent>(kf_id)
            .and_then(|kf| kf.callback.clone());

        if let Some(runtime_closure) = runtime_closure {
            if let Err(error) = eval_runtime_closure(
                &runtime_closure,
                None,
                Some(world),
                Some(rx),
                Some(kf_id),
                RuntimeClosureExecMode::KeyframeAudioOnly {
                    beat_context: kf_global_beat,
                },
            ) {
                eprintln!(
                    "[AnimationSystem] keyframe runtime closure audio lookahead failed for {kf_id:?}: {error}"
                );
            }
        }

        let action_ids: Vec<ComponentId> = world
            .children_of(kf_id)
            .iter()
            .copied()
            .filter(|&cid| world.get_component_by_id_as::<ActionComponent>(cid).is_some())
            .collect();

        for action_cid in action_ids {
            if let Err(e) = self.resolve_action_targets(world, action_cid) {
                eprintln!("[AnimationSystem] lazy resolve failed for {action_cid:?}: {e}");
                continue;
            }
            let Some(action_comp) = world.get_component_by_id_as::<ActionComponent>(action_cid)
            else {
                continue;
            };

            let mut signal = action_comp.signal.clone();
            match &mut signal {
                IntentValue::OscillatorScheduleSetPitch { beat_context, .. }
                | IntentValue::AudioSchedulePlay { beat_context, .. } => {
                    *beat_context = Some(kf_global_beat);
                    rx.push_intent_now(action_cid, signal);
                }
                _ => {}
            };
        }

        fire_music_note_children(world, rx, kf_id, Some(kf_global_beat));
    }

    pub(crate) fn evaluate_visual_due_keyframe(
        &self,
        world: &mut World,
        rx: &mut RxWorld,
        kf_id: ComponentId,
        beat_now: f64,
        audio_already_scheduled_this_cycle: bool,
    ) {
        let runtime_closure = world
            .get_component_by_id_as::<KeyframeComponent>(kf_id)
            .and_then(|kf| kf.callback.clone());

        if let Some(runtime_closure) = runtime_closure {
            if let Err(error) = eval_runtime_closure(
                &runtime_closure,
                None,
                Some(world),
                Some(rx),
                Some(kf_id),
                RuntimeClosureExecMode::KeyframeVisualOnly,
            ) {
                eprintln!(
                    "[AnimationSystem] keyframe runtime closure failed for {kf_id:?}: {error}"
                );
            }
        }

        let action_ids: Vec<ComponentId> = world
            .children_of(kf_id)
            .iter()
            .copied()
            .filter(|&cid| world.get_component_by_id_as::<ActionComponent>(cid).is_some())
            .collect();

        let mut saw_any_action = false;
        for action_cid in action_ids {
            if let Err(e) = self.resolve_action_targets(world, action_cid) {
                eprintln!("[AnimationSystem] lazy resolve failed for {action_cid:?}: {e}");
                continue;
            }
            let Some(action_comp) = world.get_component_by_id_as::<ActionComponent>(action_cid)
            else {
                continue;
            };

            saw_any_action = true;

            if audio_already_scheduled_this_cycle {
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
                    *beat_context = Some(beat_now);
                }
                _ => {}
            };

            rx.push_intent_now(action_cid, signal);
        }

        if !audio_already_scheduled_this_cycle {
            fire_music_note_children(world, rx, kf_id, Some(beat_now));
        }

        if !saw_any_action {
            if let Some(kf) = world.get_component_by_id_as::<KeyframeComponent>(kf_id) {
                println!("[AnimationSystem] beat {:.3}: (no actions)", kf.beat);
            }
        }
    }
}

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
        .filter(|&cid| world.get_component_by_id_as::<MusicNoteComponent>(cid).is_some())
        .collect();

    for note_cid in note_ids {
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
