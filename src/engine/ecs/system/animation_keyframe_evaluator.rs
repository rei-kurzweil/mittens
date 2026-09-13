use crate::engine::ecs::component::{KeyframeComponent, MusicNoteComponent};
use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;
use crate::scripting::runner::{DeferredCallbackMode, RuntimeSpecSession};

#[derive(Debug, Default)]
pub(crate) struct AnimationKeyframeEvaluator;

pub(crate) struct SessionCallbackExecutor<'a> {
    pub session: &'a mut RuntimeSpecSession,
    pub render_assets: &'a mut RenderAssets,
    pub emit: &'a mut dyn SignalEmitter,
}

impl AnimationKeyframeEvaluator {
    pub(crate) fn evaluate_audio_due_keyframe(
        &self,
        world: &mut World,
        rx: &mut RxWorld,
        kf_id: ComponentId,
        kf_global_beat: f64,
        mut executor: Option<&mut SessionCallbackExecutor<'_>>,
    ) {
        let session_callback = world
            .get_component_by_id_as::<KeyframeComponent>(kf_id)
            .and_then(|kf| kf.session_callback);

        if let Some(callback) = session_callback {
            if let Some(executor) = executor.as_mut() {
                match executor.session.invoke_deferred_callback(
                    callback,
                    DeferredCallbackMode::AudioOnly {
                        beat_context: kf_global_beat,
                    },
                    world,
                    rx,
                    Some(executor.render_assets),
                    executor.emit,
                ) {
                    Ok(intents) => {
                        for intent in intents {
                            rx.push_intent_now(ComponentId::default(), intent);
                        }
                    }
                    Err(error) => eprintln!(
                        "[AnimationSystem] session keyframe audio lookahead failed for {kf_id:?}: {error}"
                    ),
                }
            } else {
                eprintln!("[AnimationSystem] no MMS session executor for keyframe {kf_id:?}");
            }
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
        mut executor: Option<&mut SessionCallbackExecutor<'_>>,
    ) {
        let session_callback = world
            .get_component_by_id_as::<KeyframeComponent>(kf_id)
            .and_then(|kf| kf.session_callback);

        if let Some(callback) = session_callback {
            if let Some(executor) = executor.as_mut() {
                match executor.session.invoke_deferred_callback(
                    callback,
                    DeferredCallbackMode::VisualOnly,
                    world,
                    rx,
                    Some(executor.render_assets),
                    executor.emit,
                ) {
                    Ok(intents) => {
                        for intent in intents {
                            rx.push_intent_now(ComponentId::default(), intent);
                        }
                    }
                    Err(error) => eprintln!(
                        "[AnimationSystem] session keyframe callback failed for {kf_id:?}: {error}"
                    ),
                }
            } else {
                eprintln!("[AnimationSystem] no MMS session executor for keyframe {kf_id:?}");
            }
        }

        if !audio_already_scheduled_this_cycle {
            fire_music_note_children(world, rx, kf_id, Some(beat_now));
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
        .filter(|&cid| {
            world
                .get_component_by_id_as::<MusicNoteComponent>(cid)
                .is_some()
        })
        .collect();

    for note_cid in note_ids {
        let note = match world.get_component_by_id_as::<MusicNoteComponent>(note_cid) {
            Some(mn) => mn.note,
            None => continue,
        };
        rx.push_intent_now(
            note_cid,
            IntentValue::AudioSchedulePlay {
                component_id: note_cid,
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
