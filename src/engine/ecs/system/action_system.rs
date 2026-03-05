use crate::engine::ecs::component::{
    AudioBandPassFilterComponent, AudioLowPassFilterComponent, AudioOscillatorComponent,
    ColorComponent, MusicNoteComponent, RayCastComponent, RenderableComponent, TextComponent,
    TransformComponent,
};
use crate::engine::ecs::system::MusicSystem;
use crate::engine::ecs::system::audio_system::AudioOp;
use crate::engine::ecs::{
    ComponentCodec, ComponentId, RxWorld, Signal, SignalEmitter, SignalKind, SignalValue, World,
};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
pub struct ActionSystem {
    handlers_installed: bool,
}

impl ActionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        rx.add_global_handler_closure(SignalKind::Action, handle_action_signal);
        self.handlers_installed = true;
    }
}

fn handle_action_signal(world: &mut World, emit: &mut dyn SignalEmitter, env: &Signal) {
    let beat_now = 0.0;

    match &env.value {
        SignalValue::Noop => {}
        SignalValue::Print { .. } => {}

        SignalValue::SetColor { target, rgba } => {
            let mut color_cids = Vec::new();
            for &t in target.iter() {
                collect_color_targets(world, t, &mut color_cids);
            }
            color_cids.sort();
            for color_cid in color_cids {
                if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
                    c.rgba = *rgba;
                    emit.push(
                        color_cid,
                        SignalValue::RegisterColor {
                            component: color_cid,
                        },
                    );
                }
            }
        }

        SignalValue::SetText { target, text } => {
            let mut text_cids = Vec::new();
            for &t in target.iter() {
                collect_text_targets(world, t, &mut text_cids);
            }
            text_cids.sort();
            text_cids.dedup();
            for text_cid in text_cids {
                emit.push(
                    text_cid,
                    SignalValue::SetTextImmediate {
                        component: text_cid,
                        text: text.clone(),
                    },
                );
            }
        }

        SignalValue::SetPosition { target, position } => {
            let mut transform_cids = Vec::new();
            for &t in target.iter() {
                collect_transform_targets(world, t, &mut transform_cids);
            }
            transform_cids.sort();
            transform_cids.dedup();
            for transform_cid in transform_cids {
                if let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
                {
                    t.set_position(emit, position[0], position[1], position[2]);
                }
            }
        }

        SignalValue::SetTransform {
            target,
            translation,
            rotation_quat_xyzw,
            scale,
        } => {
            let mut transform_cids = Vec::new();
            for &t in target.iter() {
                collect_transform_targets(world, t, &mut transform_cids);
            }
            transform_cids.sort();
            transform_cids.dedup();
            for transform_cid in transform_cids {
                if let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
                {
                    t.transform.translation = *translation;
                    t.transform.rotation = *rotation_quat_xyzw;
                    t.transform.scale = *scale;
                    t.transform.recompute_model();
                    emit.push(
                        transform_cid,
                        SignalValue::UpdateTransform {
                            component: transform_cid,
                            translation: *translation,
                            rotation_quat_xyzw: *rotation_quat_xyzw,
                            scale: *scale,
                        },
                    );
                }
            }
        }

        SignalValue::Attach { parents, child } => {
            for &parent in parents.iter() {
                let old_parent = world.parent_of(*child);
                if let Err(e) = world.add_child(parent, *child) {
                    println!("[ActionSystem] attach failed: {e}");
                    continue;
                }

                emit.push(
                    *child,
                    SignalValue::ParentChanged {
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

                emit.push(
                    parent,
                    SignalValue::AudioGraphDirtyImmediate { component: parent },
                );
                emit.push(
                    *child,
                    SignalValue::AudioGraphDirtyImmediate { component: *child },
                );
            }
        }

        SignalValue::AttachClone {
            parents,
            prefab_root,
        } => {
            let node = match ComponentCodec::encode_subtree_node(&*world, *prefab_root) {
                Ok(n) => n,
                Err(e) => {
                    println!("[ActionSystem] attach_clone failed: {e}");
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
                        println!("[ActionSystem] attach_clone failed: {e}");
                        continue;
                    }
                };

                if world.get_component_record(new_root).is_none() {
                    println!("[ActionSystem] attach_clone: new root missing after decode");
                    continue;
                }

                if world.is_initialized(parent) {
                    world.init_component_tree(new_root, emit);
                }

                emit.push(
                    new_root,
                    SignalValue::ParentChanged {
                        child: new_root,
                        old_parent: None,
                        new_parent: Some(parent),
                    },
                );

                emit_topology_transform_refresh(world, emit, new_root);
                emit_topology_transform_refresh(world, emit, parent);

                emit.push(
                    parent,
                    SignalValue::AudioGraphDirtyImmediate { component: parent },
                );
                emit.push(
                    new_root,
                    SignalValue::AudioGraphDirtyImmediate {
                        component: new_root,
                    },
                );
            }
        }

        SignalValue::Detach { target } => {
            for &child in target.iter() {
                let old_parent = world.parent_of(child);
                world.detach_from_parent(child);

                emit.push(
                    child,
                    SignalValue::ParentChanged {
                        child,
                        old_parent,
                        new_parent: None,
                    },
                );

                if let Some(p) = old_parent {
                    emit.push(p, SignalValue::AudioGraphDirtyImmediate { component: p });
                }

                emit_topology_transform_refresh(world, emit, child);
                if let Some(p) = old_parent {
                    emit_topology_transform_refresh(world, emit, p);
                }
            }
        }

        SignalValue::RemoveChild { parents, index } => {
            for &parent in parents.iter() {
                let child = world.children_of(parent).get(*index).copied();
                let Some(child) = child else {
                    continue;
                };

                emit.push(
                    child,
                    SignalValue::AudioGraphDirtyImmediate { component: child },
                );
                emit.push(
                    parent,
                    SignalValue::AudioGraphDirtyImmediate { component: parent },
                );

                world.detach_from_parent(child);

                emit.push(
                    child,
                    SignalValue::ParentChanged {
                        child,
                        old_parent: Some(parent),
                        new_parent: None,
                    },
                );

                emit.push(child, SignalValue::RemoveSubtreeImmediate { root: child });

                emit_topology_transform_refresh(world, emit, parent);
            }
        }

        SignalValue::RemoveChildren { parents } => {
            for &parent in parents.iter() {
                let children: Vec<ComponentId> = world.children_of(parent).to_vec();
                if children.is_empty() {
                    continue;
                }

                emit.push(
                    parent,
                    SignalValue::AudioGraphDirtyImmediate { component: parent },
                );
                for child in children {
                    emit.push(
                        child,
                        SignalValue::AudioGraphDirtyImmediate { component: child },
                    );
                    world.detach_from_parent(child);

                    emit.push(
                        child,
                        SignalValue::ParentChanged {
                            child,
                            old_parent: Some(parent),
                            new_parent: None,
                        },
                    );

                    emit.push(child, SignalValue::RemoveSubtreeImmediate { root: child });
                }

                emit_topology_transform_refresh(world, emit, parent);
            }
        }

        SignalValue::RemoveSubtree { target } => {
            for &root in target.iter() {
                emit.push(
                    root,
                    SignalValue::AudioGraphDirtyImmediate { component: root },
                );
                emit.push(root, SignalValue::RemoveSubtreeImmediate { root });
            }
        }

        SignalValue::AudioGraphRebuild { target } => {
            for &t in target.iter() {
                emit.push(t, SignalValue::AudioGraphDirtyImmediate { component: t });
            }
        }

        SignalValue::RequestRaycast { target } => {
            let mut raycast_cids = Vec::new();
            for &t in target.iter() {
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

        SignalValue::AudioLowPassSetCutoffHz { target, cutoff_hz } => {
            for &t in target.iter() {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioLowPassFilterComponent>(t)
                {
                    c.cutoff_hz = if cutoff_hz.is_finite() {
                        cutoff_hz.max(0.0)
                    } else {
                        c.cutoff_hz
                    };
                    emit.push(
                        t,
                        SignalValue::ScheduleAudioOp {
                            component: t,
                            beat: beat_now,
                            op: AudioOp::SetLowPassCutoffHz(c.cutoff_hz),
                        },
                    );
                }
            }
        }

        SignalValue::AudioBandPassSetCenterHz { target, center_hz } => {
            for &t in target.iter() {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioBandPassFilterComponent>(t)
                {
                    c.center_hz = if center_hz.is_finite() {
                        center_hz.max(0.0)
                    } else {
                        c.center_hz
                    };
                    emit.push(
                        t,
                        SignalValue::ScheduleAudioOp {
                            component: t,
                            beat: beat_now,
                            op: AudioOp::SetBandPassCenterHz(c.center_hz),
                        },
                    );
                }
            }
        }

        SignalValue::OscillatorSetEnabled { target, enabled } => {
            let mut osc_cids = Vec::new();
            for &t in target.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.enabled = *enabled;
                    }
                    emit.push(
                        osc_cid,
                        SignalValue::RegisterAudioOscillator { component: osc_cid },
                    );
                }
            }
        }

        SignalValue::OscillatorSetPitch {
            target,
            frequency_hz,
        } => {
            if !frequency_hz.is_finite() {
                println!(
                    "[ActionSystem] oscillator_set_pitch: non-finite frequency_hz={frequency_hz}"
                );
                return;
            }

            let mut osc_cids = Vec::new();
            for &t in target.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.frequency = *frequency_hz;
                        osc.music_note_applied = true;
                    }
                    emit.push(
                        osc_cid,
                        SignalValue::RegisterAudioOscillator { component: osc_cid },
                    );
                }
            }
        }

        SignalValue::OscillatorScheduleSetPitch {
            target,
            beat_offset,
            beat_context,
            frequency_hz,
        } => {
            let beat = beat_context.unwrap_or(beat_now) + *beat_offset;

            let mut osc_cids = Vec::new();
            for &t in target.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push(
                    osc_cid,
                    SignalValue::ScheduleAudioPitchSetHz {
                        component: osc_cid,
                        beat,
                        frequency_hz: *frequency_hz,
                    },
                );
            }
        }

        SignalValue::OscillatorScheduleSetNote {
            target,
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
            for &t in target.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push(
                    osc_cid,
                    SignalValue::ScheduleAudioOscillatorEnabled {
                        component: osc_cid,
                        beat,
                        enabled: true,
                    },
                );
                emit.push(
                    osc_cid,
                    SignalValue::ScheduleAudioPitchSetHz {
                        component: osc_cid,
                        beat,
                        frequency_hz,
                    },
                );
                if duration_beats.is_finite() && duration_beats > 0.0 {
                    emit.push(
                        osc_cid,
                        SignalValue::ScheduleAudioOscillatorEnabled {
                            component: osc_cid,
                            beat: beat + duration_beats,
                            enabled: false,
                        },
                    );
                }
            }
        }

        SignalValue::OscillatorScheduleMusicNote {
            target,
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
            for &t in target.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push(
                    osc_cid,
                    SignalValue::ScheduleAudioOscillatorEnabled {
                        component: osc_cid,
                        beat,
                        enabled: true,
                    },
                );
                emit.push(
                    osc_cid,
                    SignalValue::ScheduleAudioPitchSetHz {
                        component: osc_cid,
                        beat,
                        frequency_hz,
                    },
                );
                emit.push(
                    osc_cid,
                    SignalValue::ScheduleAudioGainSet {
                        component: osc_cid,
                        beat,
                        gain: velocity,
                    },
                );

                if duration_beats.is_finite() && duration_beats > 0.0 {
                    emit.push(
                        osc_cid,
                        SignalValue::ScheduleAudioOscillatorEnabled {
                            component: osc_cid,
                            beat: beat + duration_beats,
                            enabled: false,
                        },
                    );
                    emit.push(
                        osc_cid,
                        SignalValue::ScheduleAudioGainSet {
                            component: osc_cid,
                            beat: beat + duration_beats,
                            gain: 1.0,
                        },
                    );
                }
            }
        }

        SignalValue::MusicSetNote { target, note } => {
            let mut osc_cids = Vec::new();
            for &t in target.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(note_cid) = find_first_music_note_component(world, osc_cid) {
                    if let Some(nc) =
                        world.get_component_by_id_as_mut::<MusicNoteComponent>(note_cid)
                    {
                        nc.note = *note;
                    }
                }

                let freq = MusicSystem::frequency_hz_for_note(*note);
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.frequency = freq;
                        osc.music_note_applied = true;
                    }
                    emit.push(
                        osc_cid,
                        SignalValue::RegisterAudioOscillator { component: osc_cid },
                    );
                }
            }
        }

        _ => {}
    }
}

fn collect_raycast_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    if world
        .get_component_by_id_as::<RayCastComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    // Subtree search: collect all RayCastComponents under this target.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<RayCastComponent>(node)
            .is_some()
        {
            out.push(node);
            continue;
        }
        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
}

fn collect_transform_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    if world
        .get_component_by_id_as::<TransformComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    // Subtree search: pick the first TransformComponent encountered per branch.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<TransformComponent>(node)
            .is_some()
        {
            out.push(node);
            continue;
        }
        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
}

fn emit_topology_transform_refresh(world: &World, emit: &mut dyn SignalEmitter, cid: ComponentId) {
    // If this node is a TransformComponent, refreshing it updates cached world matrices
    // for its whole subtree.
    if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid) {
        emit.push(
            cid,
            SignalValue::UpdateTransform {
                component: cid,
                translation: t.transform.translation,
                rotation_quat_xyzw: t.transform.rotation,
                scale: t.transform.scale,
            },
        );
        return;
    }

    // Otherwise, refresh the nearest ancestor transform (if any).
    let mut cur = cid;
    while let Some(p) = world.parent_of(cur) {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(p) {
            emit.push(
                p,
                SignalValue::UpdateTransform {
                    component: p,
                    translation: t.transform.translation,
                    rotation_quat_xyzw: t.transform.rotation,
                    scale: t.transform.scale,
                },
            );
            return;
        }
        cur = p;
    }
}

impl crate::engine::ecs::system::System for ActionSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // Signal-driven: actions are executed via RxWorld handlers.
    }
}

fn find_first_music_note_component(world: &World, target: ComponentId) -> Option<ComponentId> {
    // Find the first MusicNoteComponent anywhere in the subtree.
    if world
        .get_component_by_id_as::<MusicNoteComponent>(target)
        .is_some()
    {
        return Some(target);
    }

    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            if world
                .get_component_by_id_as::<MusicNoteComponent>(ch)
                .is_some()
            {
                return Some(ch);
            }
            stack.push(ch);
        }
    }
    None
}

fn collect_color_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    // 1) Direct ColorComponent target.
    if world
        .get_component_by_id_as::<ColorComponent>(target)
        .is_some()
    {
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

fn collect_text_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    if world
        .get_component_by_id_as::<TextComponent>(target)
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
            .get_component_by_id_as::<TextComponent>(node)
            .is_some()
        {
            out.push(node);
        }
    }
}
