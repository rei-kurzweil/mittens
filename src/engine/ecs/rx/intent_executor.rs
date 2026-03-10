use crate::engine::ecs::{ComponentId, IntentValue, Signal, SignalEmitter, World};

/// Built-in executor for **intent** signals.
///
/// This is intentionally minimal scaffolding for the ongoing refactor described in:
/// - docs/signals.md
///
/// The goal is to keep handlers observers-only, and execute side effects via intent signals.
#[derive(Debug, Default)]
pub struct RxIntentExecutor;

impl RxIntentExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether this executor is expected to handle the given signal value.
    ///
    /// Note: during migration, this is intentionally conservative; it is not yet wired into the
    /// drain loop.
    pub fn handles_value(value: &IntentValue) -> bool {
        // These are the “intent interpretation” values that expand into follow-up mutations.
        //
        // Note: `SetText` is currently executed by the default executor for text rebuilds.
        matches!(
            value,
            IntentValue::Noop
                | IntentValue::Print { .. }
                | IntentValue::SetColor { .. }
                | IntentValue::SetPosition { .. }
                | IntentValue::SetTransform { .. }
                | IntentValue::Attach { .. }
                | IntentValue::AttachClone { .. }
                | IntentValue::Detach { .. }
                | IntentValue::RemoveChild { .. }
                | IntentValue::RemoveChildren { .. }
                | IntentValue::RemoveSubtree { .. }
                | IntentValue::AudioGraphRebuild { .. }
                | IntentValue::RequestRaycast { .. }
                | IntentValue::AudioLowPassSetCutoffHz { .. }
                | IntentValue::AudioBandPassSetCenterHz { .. }
                | IntentValue::OscillatorSetEnabled { .. }
                | IntentValue::OscillatorSetPitch { .. }
                | IntentValue::OscillatorScheduleSetPitch { .. }
                | IntentValue::OscillatorScheduleSetNote { .. }
                | IntentValue::OscillatorScheduleMusicNote { .. }
                | IntentValue::MusicSetNote { .. }
        )
    }

    /// Execute an intent signal, emitting follow-up mutation signals via `emit`.
    ///
    pub fn execute(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, env: &Signal) {
        handle_intent_signal(world, emit, env);
    }
}

fn handle_intent_signal(world: &mut World, emit: &mut dyn SignalEmitter, env: &Signal) {
    use crate::engine::ecs::component::{
        AudioBandPassFilterComponent, AudioLowPassFilterComponent, AudioOscillatorComponent,
        ColorComponent, MusicNoteComponent, RayCastComponent, TransformComponent,
    };
    use crate::engine::ecs::system::MusicSystem;
    use crate::engine::ecs::system::audio_system::AudioOp;
    use crate::engine::ecs::{ComponentCodec, ComponentId, EventSignal, IntentValue};

    let beat_now = 0.0;

    let Some(intent) = env.intent.as_ref() else {
        return;
    };

    match &intent.value {
        IntentValue::Noop => {}
        IntentValue::Print { .. } => {}

        IntentValue::SetColor { component_ids, rgba } => {
            let mut color_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_color_targets(world, t, &mut color_cids);
            }
            color_cids.sort();
            for color_cid in color_cids {
                if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
                    c.rgba = *rgba;
                    emit.push_intent_now(
                        color_cid,
                        IntentValue::RegisterColor {
                            component_ids: vec![color_cid],
                        },
                    );
                }
            }
        }

        IntentValue::SetText { .. } => {
            // Executed by the mutation executor.
        }

        IntentValue::SetPosition {
            component_ids,
            position,
        } => {
            let mut transform_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_transform_targets(world, t, &mut transform_cids);
            }
            transform_cids.sort();
            transform_cids.dedup();
            for transform_cid in transform_cids {
                if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
                {
                    t.set_position(emit, position[0], position[1], position[2]);
                }
            }
        }

        IntentValue::SetTransform {
            component_ids,
            translation,
            rotation_quat_xyzw,
            scale,
        } => {
            let mut transform_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_transform_targets(world, t, &mut transform_cids);
            }
            transform_cids.sort();
            transform_cids.dedup();
            for transform_cid in transform_cids {
                if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
                {
                    t.transform.translation = *translation;
                    t.transform.rotation = *rotation_quat_xyzw;
                    t.transform.scale = *scale;
                    t.transform.recompute_model();
                    emit.push_intent_now(
                        transform_cid,
                        IntentValue::UpdateTransform {
                            component_ids: vec![transform_cid],
                            translation: *translation,
                            rotation_quat_xyzw: *rotation_quat_xyzw,
                            scale: *scale,
                        },
                    );
                }
            }
        }

        IntentValue::Attach { parents, child } => {
            for &parent in parents.iter() {
                let old_parent = world.parent_of(*child);
                if let Err(e) = world.add_child(parent, *child) {
                    println!("[IntentExecutor] attach failed: {e}");
                    continue;
                }

                emit.push_event(
                    *child,
                    EventSignal::ParentChanged {
                        child: *child,
                        old_parent,
                        new_parent: Some(parent),
                    },
                );

                if world.is_initialized(parent) {
                    world.init_component_tree(*child, emit);
                }

                emit_topology_transform_refresh(world, emit, *child);
                emit_topology_transform_refresh(world, emit, parent);

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                emit.push_intent_now(
                    *child,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![*child],
                    },
                );
            }
        }

        IntentValue::AttachClone { parents, prefab_root } => {
            let node = match ComponentCodec::encode_subtree_node(&*world, *prefab_root) {
                Ok(n) => n,
                Err(e) => {
                    println!("[IntentExecutor] attach_clone failed: {e}");
                    return;
                }
            };

            for &parent in parents.iter() {
                let new_root = match ComponentCodec::decode_subtree_node_with_new_guids(
                    world,
                    Some(parent),
                    &node,
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IntentExecutor] attach_clone failed: {e}");
                        continue;
                    }
                };

                if world.get_component_record(new_root).is_none() {
                    println!("[IntentExecutor] attach_clone: new root missing after decode");
                    continue;
                }

                if world.is_initialized(parent) {
                    world.init_component_tree(new_root, emit);
                }

                emit.push_event(
                    new_root,
                    EventSignal::ParentChanged {
                        child: new_root,
                        old_parent: None,
                        new_parent: Some(parent),
                    },
                );

                emit_topology_transform_refresh(world, emit, new_root);
                emit_topology_transform_refresh(world, emit, parent);

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                emit.push_intent_now(
                    new_root,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![new_root],
                    },
                );
            }
        }

        IntentValue::Detach { component_ids } => {
            for &child in component_ids.iter() {
                let old_parent = world.parent_of(child);
                world.detach_from_parent(child);

                emit.push_event(
                    child,
                    EventSignal::ParentChanged {
                        child,
                        old_parent,
                        new_parent: None,
                    },
                );

                if let Some(p) = old_parent {
                    emit.push_intent_now(
                        p,
                        IntentValue::AudioGraphDirtyImmediate {
                            component_ids: vec![p],
                        },
                    );
                }

                emit_topology_transform_refresh(world, emit, child);
                if let Some(p) = old_parent {
                    emit_topology_transform_refresh(world, emit, p);
                }
            }
        }

        IntentValue::RemoveChild { parents, index } => {
            for &parent in parents.iter() {
                let child = world.children_of(parent).get(*index).copied();
                let Some(child) = child else {
                    continue;
                };

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                emit.push_intent_now(
                    child,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![child],
                    },
                );

                world.detach_from_parent(child);

                emit.push_event(
                    child,
                    EventSignal::ParentChanged {
                        child,
                        old_parent: Some(parent),
                        new_parent: None,
                    },
                );

                emit.push_intent_now(
                    child,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![child],
                    },
                );

                emit_topology_transform_refresh(world, emit, parent);
            }
        }

        IntentValue::RemoveChildren { parents } => {
            for &parent in parents.iter() {
                let children: Vec<ComponentId> = world.children_of(parent).to_vec();
                if children.is_empty() {
                    continue;
                }

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                for child in children {
                    emit.push_intent_now(
                        child,
                        IntentValue::AudioGraphDirtyImmediate {
                            component_ids: vec![child],
                        },
                    );

                    world.detach_from_parent(child);
                    emit.push_event(
                        child,
                        EventSignal::ParentChanged {
                            child,
                            old_parent: Some(parent),
                            new_parent: None,
                        },
                    );

                    emit.push_intent_now(
                        child,
                        IntentValue::RemoveSubtree {
                            component_ids: vec![child],
                        },
                    );
                }

                emit_topology_transform_refresh(world, emit, parent);
            }
        }

        IntentValue::RemoveSubtree { component_ids } => {
            for &root in component_ids.iter() {
                emit.push_intent_now(
                    root,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![root],
                    },
                );
            }
        }

        IntentValue::AudioGraphRebuild { component_ids } => {
            for &t in component_ids.iter() {
                emit.push_intent_now(
                    t,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![t],
                    },
                );
            }
        }

        IntentValue::RequestRaycast { component_ids } => {
            let mut raycast_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_raycast_targets(world, t, &mut raycast_cids);
            }
            raycast_cids.sort();
            raycast_cids.dedup();

            for rcid in raycast_cids {
                if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(rcid) {
                    rc.cast_requests = rc.cast_requests.saturating_add(1);
                }
            }
        }

        IntentValue::AudioLowPassSetCutoffHz {
            component_ids,
            cutoff_hz,
        } => {
            for &t in component_ids.iter() {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioLowPassFilterComponent>(t)
                {
                    c.cutoff_hz = if cutoff_hz.is_finite() {
                        cutoff_hz.max(0.0)
                    } else {
                        c.cutoff_hz
                    };
                    emit.push_intent_now(
                        t,
                        IntentValue::ScheduleAudioOp {
                            component_ids: vec![t],
                            beat: beat_now,
                            op: AudioOp::SetLowPassCutoffHz(c.cutoff_hz),
                        },
                    );
                }
            }
        }

        IntentValue::AudioBandPassSetCenterHz {
            component_ids,
            center_hz,
        } => {
            for &t in component_ids.iter() {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioBandPassFilterComponent>(t)
                {
                    c.center_hz = if center_hz.is_finite() {
                        center_hz.max(0.0)
                    } else {
                        c.center_hz
                    };
                    emit.push_intent_now(
                        t,
                        IntentValue::ScheduleAudioOp {
                            component_ids: vec![t],
                            beat: beat_now,
                            op: AudioOp::SetBandPassCenterHz(c.center_hz),
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorSetEnabled {
            component_ids,
            enabled,
        } => {
            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.enabled = *enabled;
                    }
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::RegisterAudioOscillator {
                            component_ids: vec![osc_cid],
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorSetPitch {
            component_ids,
            frequency_hz,
        } => {
            if !frequency_hz.is_finite() {
                println!(
                    "[IntentExecutor] oscillator_set_pitch: non-finite frequency_hz={frequency_hz}"
                );
                return;
            }

            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.frequency = *frequency_hz;
                        osc.music_note_applied = true;
                    }
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::RegisterAudioOscillator {
                            component_ids: vec![osc_cid],
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorScheduleSetPitch {
            component_ids,
            beat_offset,
            beat_context,
            frequency_hz,
        } => {
            let beat = beat_context.unwrap_or(beat_now) + *beat_offset;

            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioPitchSetHz {
                        component_ids: vec![osc_cid],
                        beat,
                        frequency_hz: *frequency_hz,
                    },
                );
            }
        }

        IntentValue::OscillatorScheduleSetNote {
            component_ids,
            beat_offset,
            beat_context,
            pitch,
            octave,
            duration_beats,
        } => {
            let duration_beats = (*duration_beats).max(0.0) as f64;
            let beat = beat_context.unwrap_or(beat_now) + *beat_offset;

            let note = crate::engine::ecs::component::MusicNote::from_pitch(
                duration_beats as f32,
                *pitch,
                *octave,
            );
            let frequency_hz = MusicSystem::frequency_hz_for_note(note);

            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioOscillatorEnabled {
                        component_ids: vec![osc_cid],
                        beat,
                        enabled: true,
                    },
                );
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioPitchSetHz {
                        component_ids: vec![osc_cid],
                        beat,
                        frequency_hz,
                    },
                );
                if duration_beats.is_finite() && duration_beats > 0.0 {
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::ScheduleAudioOscillatorEnabled {
                            component_ids: vec![osc_cid],
                            beat: beat + duration_beats,
                            enabled: false,
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorScheduleMusicNote {
            component_ids,
            beat_offset,
            beat_context,
            note,
        } => {
            let velocity = note.velocity();
            let velocity = if velocity.is_finite() {
                velocity.max(0.0)
            } else {
                1.0
            };

            let frequency_hz = MusicSystem::frequency_hz_for_note(*note);
            let duration_beats = note.duration_beats() as f64;
            let beat = beat_context.unwrap_or(beat_now) + *beat_offset;

            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioOscillatorEnabled {
                        component_ids: vec![osc_cid],
                        beat,
                        enabled: true,
                    },
                );
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioPitchSetHz {
                        component_ids: vec![osc_cid],
                        beat,
                        frequency_hz,
                    },
                );
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioGainSet {
                        component_ids: vec![osc_cid],
                        beat,
                        gain: velocity,
                    },
                );

                if duration_beats.is_finite() && duration_beats > 0.0 {
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::ScheduleAudioOscillatorEnabled {
                            component_ids: vec![osc_cid],
                            beat: beat + duration_beats,
                            enabled: false,
                        },
                    );
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::ScheduleAudioGainSet {
                            component_ids: vec![osc_cid],
                            beat: beat + duration_beats,
                            gain: 1.0,
                        },
                    );
                }
            }
        }

        IntentValue::MusicSetNote {
            component_ids,
            note,
        } => {
            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(note_cid) = find_first_music_note_component(world, osc_cid) {
                    if let Some(nc) = world.get_component_by_id_as_mut::<MusicNoteComponent>(note_cid)
                    {
                        nc.note = *note;
                    }
                }

                let freq = MusicSystem::frequency_hz_for_note(*note);
                if let Some(c) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.frequency = freq;
                        osc.music_note_applied = true;
                    }
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::RegisterAudioOscillator {
                            component_ids: vec![osc_cid],
                        },
                    );
                }
            }
        }

        _ => {}
    }
}

fn collect_raycast_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::RayCastComponent;

    if world.get_component_by_id_as::<RayCastComponent>(target).is_some() {
        out.push(target);
        return;
    }

    // Subtree search: collect all RayCastComponents under this target.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        if world.get_component_by_id_as::<RayCastComponent>(node).is_some() {
            out.push(node);
            continue;
        }
        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
}

fn collect_transform_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::TransformComponent;

    if world.get_component_by_id_as::<TransformComponent>(target).is_some() {
        out.push(target);
        return;
    }

    // Subtree search: pick the first TransformComponent encountered per branch.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        if world.get_component_by_id_as::<TransformComponent>(node).is_some() {
            out.push(node);
            continue;
        }
        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
}

fn emit_topology_transform_refresh(world: &World, emit: &mut dyn SignalEmitter, cid: ComponentId) {
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::ecs::IntentValue;

    // If this node is a TransformComponent, refreshing it updates cached world matrices
    // for its whole subtree.
    if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid) {
        let _ = t;
        emit.push_intent_now(
            cid,
            IntentValue::RefreshTransform {
                component_ids: vec![cid],
            },
        );
        return;
    }

    // Otherwise, refresh the nearest ancestor transform (if any).
    let mut cur = cid;
    while let Some(p) = world.parent_of(cur) {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(p) {
            let _ = t;
            emit.push_intent_now(
                p,
                IntentValue::RefreshTransform {
                    component_ids: vec![p],
                },
            );
            return;
        }
        cur = p;
    }
}

fn find_first_music_note_component(world: &World, target: ComponentId) -> Option<ComponentId> {
    use crate::engine::ecs::component::MusicNoteComponent;

    // Find the first MusicNoteComponent anywhere in the subtree.
    if world.get_component_by_id_as::<MusicNoteComponent>(target).is_some() {
        return Some(target);
    }

    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            if world.get_component_by_id_as::<MusicNoteComponent>(ch).is_some() {
                return Some(ch);
            }
            stack.push(ch);
        }
    }
    None
}

fn collect_color_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::{ColorComponent, RenderableComponent};

    // 1) Direct ColorComponent target.
    if world.get_component_by_id_as::<ColorComponent>(target).is_some() {
        out.push(target);
        return;
    }

    // 2) RenderableComponent target -> find immediate ColorComponent child.
    if world
        .get_component_by_id_as::<RenderableComponent>(target)
        .is_some()
    {
        for &ch in world.children_of(target) {
            if world.get_component_by_id_as::<ColorComponent>(ch).is_some() {
                out.push(ch);
                return;
            }
        }
        return;
    }

    // 3) Generic subtree target (e.g. TransformComponent): search for renderables and their color children.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }

        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            for &ch in world.children_of(node) {
                if world.get_component_by_id_as::<ColorComponent>(ch).is_some() {
                    out.push(ch);
                }
            }
        }
    }
}

fn collect_oscillator_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::AudioOscillatorComponent;

    if world
        .get_component_by_id_as::<AudioOscillatorComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }

        if world
            .get_component_by_id_as::<AudioOscillatorComponent>(node)
            .is_some()
        {
            out.push(node);
        }
    }
}
