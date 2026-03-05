//! Per-frame context + legacy name: used to be a command queue.
//!
//! This is a transitional facade that exists mainly to thread per-frame transport context
//! (beat/bpm) through systems.
//!
//! Important safety note:
//! - This type must not store raw pointers into `SystemWorld`/`RxWorld` owned alongside it
//!   (e.g. in `Universe`), because moving that owner would invalidate the pointer.
//! - Instead, we queue signals locally and drain them into `SystemWorld.rx` at explicit
//!   drain points.

use crate::engine::ecs::{ComponentId, RxWorld, Signal, SignalEmitter, SignalValue, SignalWhen};

pub struct CommandQueue {
    queued: Vec<Signal>,

    // Per-frame transport context.
    frame_beat_now: f64,
    frame_bpm: f64,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            queued: Vec::new(),
            frame_beat_now: 0.0,
            frame_bpm: 120.0,
        }
    }

    /// Drain locally-queued signals into the target `RxWorld`.
    ///
    /// Returns the number of signals moved.
    pub fn drain_into_rx(&mut self, rx: &mut RxWorld) -> usize {
        if self.queued.is_empty() {
            return 0;
        }

        let drained = std::mem::take(&mut self.queued);
        let moved = drained.len();
        for env in drained {
            match env.when {
                SignalWhen::Now => rx.push(env.scope, env.value),
                SignalWhen::AtBeat(beat) => rx.push_at_beat(env.scope, beat, env.value),
            }
        }
        moved
    }

    pub fn set_transport(&mut self, beat_now: f64, bpm: f64) {
        self.frame_beat_now = beat_now;
        self.frame_bpm = bpm;
    }

    pub fn beat_now(&self) -> f64 {
        self.frame_beat_now
    }

    pub fn bpm(&self) -> f64 {
        self.frame_bpm
    }

    // --- Former command-queue API (now: emits typed SignalValue actions) ---

    pub fn register_renderable(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterRenderable { component });
    }

    pub fn remove_renderable(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RemoveRenderable { component });
    }

    pub fn register_transform(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterTransform { component });
    }

    pub fn update_transform(
        &mut self,
        component: ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    ) {
        self.push(
            component,
            SignalValue::UpdateTransform {
                component,
                translation: transform.translation,
                rotation_quat_xyzw: transform.rotation,
                scale: transform.scale,
            },
        );
    }

    pub fn register_camera_3d(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterCamera3d { component });
    }

    pub fn register_camera2d(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterCamera2d { component });
    }

    pub fn make_active_camera(&mut self, component: ComponentId) {
        self.push(component, SignalValue::MakeActiveCamera { component });
    }

    pub fn register_input(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterInput { component });
    }

    pub fn register_uv(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterUv { component });
    }

    pub fn register_light(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterLight { component });
    }

    pub fn register_color(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterColor { component });
    }

    pub fn register_opacity(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterOpacity { component });
    }

    pub fn register_transparent_cutout(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterTransparentCutout { component },
        );
    }

    pub fn register_background_color(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterBackgroundColor { component },
        );
    }

    pub fn register_ambient_light(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterAmbientLight { component });
    }

    pub fn register_texture(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterTexture { component });
    }

    pub fn register_texture_filtering(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterTextureFiltering { component },
        );
    }

    pub fn register_text(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterText { component });
    }

    pub fn set_text(&mut self, component: ComponentId, text: String) {
        self.push(component, SignalValue::SetTextImmediate { component, text });
    }

    pub fn register_emissive(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterEmissive { component });
    }

    pub fn register_light_quantization(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterLightQuantization { component },
        );
    }

    pub fn register_collision(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterCollision { component });
    }

    pub fn register_kinetic_response(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterKineticResponse { component },
        );
    }

    pub fn remove_kinetic_response(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RemoveKineticResponse { component });
    }

    pub fn remove_collision(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RemoveCollision { component });
    }

    pub fn remove_subtree(&mut self, root: ComponentId) {
        self.push(root, SignalValue::RemoveSubtreeImmediate { root });
    }

    pub fn register_openxr(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterOpenxr { component });
    }

    pub fn register_controller_xr(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterControllerXr { component });
    }

    pub fn remove_controller_xr(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RemoveControllerXr { component });
    }

    pub fn register_raycast(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterRaycast { component });
    }

    pub fn remove_raycast(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RemoveRaycast { component });
    }

    pub fn register_animation(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterAnimation { component });
    }

    pub fn register_keyframe(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterKeyframe { component });
    }

    pub fn register_audio_output(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterAudioOutput { component });
    }

    pub fn audio_graph_dirty(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::AudioGraphDirtyImmediate { component },
        );
    }

    pub fn register_audio_oscillator(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterAudioOscillator { component },
        );
    }

    pub fn register_audio_buffer_size(&mut self, component: ComponentId) {
        self.push(
            component,
            SignalValue::RegisterAudioBufferSize { component },
        );
    }

    pub fn register_clock(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterClock { component });
    }

    pub fn register_transform_gizmo(&mut self, component: ComponentId) {
        self.push(component, SignalValue::RegisterTransformGizmo { component });
    }

    /// Flush used to apply queued commands; now it executes pending signals.
    pub fn flush(
        &mut self,
        world: &mut crate::engine::ecs::World,
        systems: &mut crate::engine::ecs::system::SystemWorld,
        visuals: &mut crate::engine::graphics::VisualWorld,
    ) {
        // Execute + dispatch any newly-pushed signals.
        let _ = systems.process_signals(world, visuals, self, 100_000);
    }

    // --- Deprecated / transitional (should not be used once ActionSystem stops queuing AudioOps) ---

    #[allow(dead_code)]
    pub fn schedule_audio_op(
        &mut self,
        _component_id: ComponentId,
        _beat: f64,
        _op: crate::engine::ecs::system::audio_system::AudioOp,
    ) {
        unimplemented!("schedule_audio_op: convert to direct audio scheduling in the executor");
    }

    #[allow(dead_code)]
    pub fn schedule_audio_graph_swap(&mut self, _component_id: ComponentId, _beat: f64) {
        unimplemented!(
            "schedule_audio_graph_swap: convert to direct audio scheduling in the executor"
        );
    }

    #[allow(dead_code)]
    pub fn schedule_audio_pitch_set_hz(
        &mut self,
        _component_id: ComponentId,
        _beat: f64,
        _frequency_hz: f32,
    ) {
        unimplemented!(
            "schedule_audio_pitch_set_hz: convert to direct audio scheduling in the executor"
        );
    }

    #[allow(dead_code)]
    pub fn schedule_audio_oscillator_enabled(
        &mut self,
        _component_id: ComponentId,
        _beat: f64,
        _enabled: bool,
    ) {
        unimplemented!(
            "schedule_audio_oscillator_enabled: convert to direct audio scheduling in the executor"
        );
    }

    #[allow(dead_code)]
    pub fn schedule_audio_gain_set(&mut self, _component_id: ComponentId, _beat: f64, _gain: f32) {
        unimplemented!(
            "schedule_audio_gain_set: convert to direct audio scheduling in the executor"
        );
    }
}

impl SignalEmitter for CommandQueue {
    fn push(&mut self, scope: ComponentId, value: SignalValue) {
        self.queued.push(Signal {
            scope,
            value,
            when: SignalWhen::Now,
        });
    }

    fn push_at_beat(&mut self, scope: ComponentId, beat: f64, value: SignalValue) {
        if !beat.is_finite() {
            self.push(scope, value);
            return;
        }

        self.queued.push(Signal {
            scope,
            value,
            when: SignalWhen::AtBeat(beat),
        });
    }
}
