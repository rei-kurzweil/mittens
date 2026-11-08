//! Opt-in test-only update timings. Fixed counters; no logging in timed stages.
use meow_meow_script as mms;
use std::{cell::RefCell, time::Instant};
#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum Phase {
    UniverseStart,
    SystemsTick,
    RuntimeCallbacks,
    FinalCommands,
    UniverseTail,
    SystemInput,
    GltfImports,
    PreFrameFlush,
    FrameDispatch,
    VisualsAudioClock,
    Animation,
    AnimationFlush,
    TransformOperators,
    TransformTick,
    Skinning,
    Bvh,
    ContactBeforeVelocity,
    VelocityIntegrate,
    VelocityFlush,
    VelocityContact,
    CameraXr,
    XrInputContact,
    RayGestureInput,
    AvatarIk,
    HandTracking,
    SecondaryMotion,
    SpringPropagation,
    SpringVisualsSkin,
    LayoutTail,
    VisualPublication,
    HostQuery,
    HostValidation,
    HostMethod,
    HostOther,
    CallbackLeaseEnter,
    CallbackInvoke,
    CallbackLeaseExit,
}
const COUNT: usize = 37;
const NAMES: [&str; COUNT] = [
    "universe_start",
    "systems_tick_total",
    "runtime_callbacks",
    "final_commands",
    "universe_tail",
    "input_and_signals",
    "gltf_imports_and_flush",
    "pre_frame_flush",
    "frame_tick_dispatch",
    "visualization_audio_clock",
    "animation",
    "animation_signals_transitions",
    "transform_operators",
    "transform_tick",
    "initial_skinning",
    "initial_bvh",
    "contact_before_velocity",
    "velocity_integrate",
    "velocity_command_flush",
    "velocity_contacts",
    "camera_xr",
    "xr_input_and_contact",
    "ray_gesture_input",
    "avatar_ik_and_flush",
    "hand_tracking",
    "secondary_motion",
    "spring_propagation",
    "spring_visuals_and_skinning",
    "layout_and_final_flush",
    "visual_publication",
    "host_query",
    "host_validation",
    "host_method",
    "host_other",
    "callback_lease_enter",
    "callback_invoke",
    "callback_lease_exit",
];
struct State {
    enabled: bool,
    milliseconds: [f64; COUNT],
    velocity_steps: usize,
    counters: [u64; 8],
    callback_labels: std::collections::BTreeMap<u64, String>,
    callback_samples: std::collections::BTreeMap<u64, (u64, f64)>,
}
thread_local! {static STATE: RefCell<State> = const {RefCell::new(State {enabled:false,milliseconds:[0.;COUNT],velocity_steps:0,counters:[0;8],callback_labels:std::collections::BTreeMap::new(),callback_samples:std::collections::BTreeMap::new()})};}
pub(crate) fn enable(enabled: bool) {
    STATE.with_borrow_mut(|s| s.enabled = enabled);
}
pub(crate) fn velocity_steps(count: usize) {
    STATE.with_borrow_mut(|s| {
        if s.enabled {
            s.velocity_steps += count;
        }
    });
}
pub(crate) fn snapshot() -> serde_json::Value {
    STATE.with_borrow(|s|serde_json::json!({"stages_ms":NAMES.iter().enumerate().map(|(i,&name)|(name,s.milliseconds[i])).collect::<std::collections::BTreeMap<_,_>>(),"velocity_steps":s.velocity_steps,"host_counters":s.counters,"callbacks":s.callback_samples.iter().filter(|(_,v)|v.0>0).map(|(id,&(count,ms))|serde_json::json!({"id":id,"label":s.callback_labels.get(id),"count":count,"ms":ms})).collect::<Vec<_>>()}))
}
pub(crate) struct Timer {
    last: Option<Instant>,
}
impl Timer {
    pub(crate) fn begin_frame() -> Self {
        STATE.with_borrow_mut(|s| {
            s.milliseconds.fill(0.);
            s.velocity_steps = 0;
            s.counters.fill(0);
            for sample in s.callback_samples.values_mut() {
                *sample = (0, 0.);
            }
        });
        Self::new()
    }
    pub(crate) fn new() -> Self {
        Self {
            last: STATE.with_borrow(|s| s.enabled.then(Instant::now)),
        }
    }
    pub(crate) fn mark(&mut self, phase: Phase) {
        if let Some(previous) = self.last {
            let now = Instant::now();
            self.last = Some(now);
            STATE.with_borrow_mut(|s| {
                s.milliseconds[phase as usize] += now.duration_since(previous).as_secs_f64() * 1000.
            });
        }
    }
}

// Counter order: callbacks, queries, validations, validated handles, methods,
// other requests, pickup queries. Host timings nest inside callback timings.
pub(crate) fn callbacks(count: usize) {
    STATE.with_borrow_mut(|s| {
        if s.enabled {
            s.counters[0] += count as u64;
        }
    });
}
pub(crate) struct Scope {
    started: Option<Instant>,
    phase: Phase,
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_secs_f64() * 1000.;
            STATE.with_borrow_mut(|s| s.milliseconds[self.phase as usize] += elapsed);
        }
    }
}
pub(crate) fn host_request(request: &mms::HostRequest) -> Scope {
    let phase = STATE.with_borrow_mut(|s| {
        if !s.enabled {
            return Phase::HostOther;
        }
        match request {
            mms::HostRequest::Query { selector, .. } => {
                s.counters[1] += 1;
                if selector == "#pickup_truck" {
                    s.counters[6] += 1;
                }
                Phase::HostQuery
            }
            mms::HostRequest::ValidateComponents { components } => {
                s.counters[2] += 1;
                s.counters[3] += components.len() as u64;
                Phase::HostValidation
            }
            mms::HostRequest::InvokeComponentMethod { .. }
            | mms::HostRequest::InvokeComponentMethodByName { .. } => {
                s.counters[4] += 1;
                Phase::HostMethod
            }
            _ => {
                s.counters[5] += 1;
                Phase::HostOther
            }
        }
    });
    Scope {
        started: STATE.with_borrow(|s| s.enabled.then(Instant::now)),
        phase,
    }
}

pub(crate) fn register_callback(id: u64, label: impl FnOnce() -> String) {
    STATE.with_borrow_mut(|s| {
        if s.enabled {
            s.callback_labels.insert(id, label());
        }
    });
}
pub(crate) struct CallbackScope {
    id: u64,
    started: Option<Instant>,
}
pub(crate) fn callback_scope(id: u64, args: &[mms::TransportValue]) -> CallbackScope {
    let enabled = STATE.with_borrow_mut(|s| {
        if s.enabled {
            s.callback_samples.entry(id).or_default();
            if matches!(args.first(),Some(mms::TransportValue::String(name)) if name=="VelocityChanged") {s.counters[7]+=1;}
        }
        s.enabled
    });
    CallbackScope {
        id,
        started: enabled.then(Instant::now),
    }
}
impl Drop for CallbackScope {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed().as_secs_f64() * 1000.;
            STATE.with_borrow_mut(|s| {
                let sample = s.callback_samples.get_mut(&self.id).unwrap();
                sample.0 += 1;
                sample.1 += elapsed;
            });
        }
    }
}
