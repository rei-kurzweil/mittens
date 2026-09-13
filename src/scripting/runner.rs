use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meow_meow_script as mms;

use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, World};
use crate::engine::graphics::render_assets::RenderAssets;
use crate::scripting::object::{HeapHandle, MaterializedCE, Value};
use crate::scripting::world_evaluator::{
    EvalRequest, EvalResponse, HostCallKind, HostValue, MeowMeowEvaluator, eval_mms_fn,
    eval_module_source,
};

/// The result of evaluating an MMS script: collected intents and any errors.
#[derive(Debug, Default)]
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct IdleMittensHost;

impl mms::Host for IdleMittensHost {}

/// A crate-runtime MMS session retained between engine frames.
///
/// The idle session owns script scopes, tables, and callbacks without
/// borrowing the engine. `service_callbacks` lends it a live Mittens host only
/// for the duration of queued callback evaluation.
pub struct RuntimeSpecSession {
    configured: Arc<crate::scripting::runtime_config::MittensRuntime>,
    session: Option<mms::Session<IdleMittensHost>>,
    callback_invocations: Arc<Mutex<Vec<mms::CallbackInvocation>>>,
    callback_delivery_enabled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeferredCallbackMode {
    AudioOnly { beat_context: f64 },
    VisualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeferredCallbackError {
    ClosedSession,
    ForeignSession,
    StaleCallback,
    Evaluation(String),
}

impl std::fmt::Display for DeferredCallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClosedSession => f.write_str("Mittens RuntimeSpec session is closed"),
            Self::ForeignSession => f.write_str("deferred callback belongs to another MMS session"),
            Self::StaleCallback => f.write_str("deferred callback is stale"),
            Self::Evaluation(error) => f.write_str(error),
        }
    }
}

impl RuntimeSpecSession {
    /// Invoke a retained keyframe callback only in a phase its session-owned
    /// profile permits.  The engine supplies the opaque reference and plain
    /// metadata only; it never inspects or re-analyzes the callback body.
    pub fn invoke_keyframe_callback(
        &mut self,
        callback_ref: mms::SessionCallbackRef,
        effect_profile: mms::KeyframeEffectProfile,
        mode: DeferredCallbackMode,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<Vec<IntentValue>, DeferredCallbackError> {
        let permitted = match mode {
            DeferredCallbackMode::AudioOnly { .. } => effect_profile.runs_in_audio_phase(),
            DeferredCallbackMode::VisualOnly => effect_profile.runs_in_visual_phase(),
        };
        if !permitted {
            return Ok(Vec::new());
        }
        self.invoke_deferred_callback(callback_ref, mode, world, rx, render_assets, emit)
    }

    /// Start a retained MMS execution using the default Mittens runtime.
    ///
    /// This compatibility convenience preserves the original immediate
    /// intent result. New callers that need a canonical root source identity
    /// should use [`Self::start_at_path`] or [`Self::start_with_source_id`].
    pub fn start(
        source: &str,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(Self, Vec<IntentValue>), String> {
        let configured = Arc::new(
            crate::scripting::runtime_config::build_mittens_runtime()
                .map_err(|error| format!("Mittens RuntimeSpec build failed: {error}"))?,
        );
        let (session, output) =
            Self::start_with_runtime(configured, source, None, world, rx, render_assets, emit)?;
        Ok((session, output.intents))
    }

    /// Start a retained execution whose root source is `path`.
    ///
    /// The path is canonicalized before evaluation, so nested imports are
    /// resolved relative to the source rather than the process working
    /// directory.
    pub fn start_at_path(
        source: &str,
        path: &str,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(Self, EvalOutput), String> {
        Self::start_with_source_id(
            source,
            Some(source_identity(path)?),
            world,
            rx,
            render_assets,
            emit,
        )
    }

    /// Start a retained execution with an explicit canonical root identity.
    ///
    /// Pass `None` only for source that cannot import relative modules.
    pub fn start_with_source_id(
        source: &str,
        source_id: Option<mms::SourceId>,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(Self, EvalOutput), String> {
        let configured = Arc::new(
            crate::scripting::runtime_config::build_mittens_runtime()
                .map_err(|error| format!("Mittens RuntimeSpec build failed: {error}"))?,
        );
        Self::start_with_runtime(
            configured,
            source,
            source_id,
            world,
            rx,
            render_assets,
            emit,
        )
    }

    /// Start a retained execution from a caller-provided configured runtime.
    ///
    /// A configured runtime is immutable and may be shared by independent
    /// sessions. Each returned session retains its own callbacks, module
    /// cache, captured tables, and source identity.
    pub fn start_with_runtime(
        configured: Arc<crate::scripting::runtime_config::MittensRuntime>,
        source: &str,
        source_id: Option<mms::SourceId>,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(Self, EvalOutput), String> {
        let callback_invocations = Arc::new(Mutex::new(Vec::new()));
        let callback_delivery_enabled = Arc::new(AtomicBool::new(true));
        let mut intents = Vec::new();
        let mut host = crate::scripting::host::MittensHost::new(world, emit, &mut intents)
            .with_rx(rx)
            .with_bindings(configured.bindings())
            .with_callback_invocations(Arc::clone(&callback_invocations))
            .with_callback_delivery_enabled(Arc::clone(&callback_delivery_enabled));
        if let Some(render_assets) = render_assets {
            host = host.with_render_assets(render_assets);
        }
        let session = configured.runtime().session(IdleMittensHost);
        let (session, evaluation) = session.with_host(host, |session| {
            session.eval_with_source_id(source, source_id)
        });
        evaluation.map_err(|error| error.to_string())?;

        Ok((
            Self {
                configured,
                session: Some(session),
                callback_invocations,
                callback_delivery_enabled,
            },
            EvalOutput {
                intents,
                errors: Vec::new(),
            },
        ))
    }

    /// Disable callback delivery and release the crate-owned session state.
    ///
    /// Existing engine-side Rx registrations become inert immediately. Their
    /// physical removal remains the responsibility of the owning engine
    /// lifecycle, but they cannot enqueue or execute callbacks after close.
    pub fn close(&mut self) {
        self.callback_delivery_enabled
            .store(false, Ordering::Release);
        self.callback_invocations.lock().unwrap().clear();
        self.session = None;
    }

    pub fn is_closed(&self) -> bool {
        self.session.is_none()
    }

    /// Invoke one component-owned callback immediately through its originating
    /// session. Calls raised by this callback remain in the ordinary queue and
    /// are not recursively serviced here.
    pub fn invoke_deferred_callback(
        &mut self,
        callback_ref: mms::SessionCallbackRef,
        mode: DeferredCallbackMode,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<Vec<IntentValue>, DeferredCallbackError> {
        let Some(idle) = self.session.take() else {
            return Err(DeferredCallbackError::ClosedSession);
        };
        if idle.handle() != callback_ref.session {
            self.session = Some(idle);
            return Err(DeferredCallbackError::ForeignSession);
        }

        let mut intents = Vec::new();
        let host_phase = match mode {
            DeferredCallbackMode::AudioOnly { .. } => {
                crate::scripting::host::DeferredCallbackHostPhase::Audio
            }
            DeferredCallbackMode::VisualOnly => {
                crate::scripting::host::DeferredCallbackHostPhase::Visual
            }
        };
        let mut host = crate::scripting::host::MittensHost::new(world, emit, &mut intents)
            .with_rx(rx)
            .with_bindings(self.configured.bindings())
            .with_callback_invocations(Arc::clone(&self.callback_invocations))
            .with_callback_delivery_enabled(Arc::clone(&self.callback_delivery_enabled))
            .with_deferred_callback_phase(host_phase);
        if let Some(render_assets) = render_assets {
            host = host.with_render_assets(render_assets);
        }
        let (idle, result) = idle.with_host(host, |session| {
            session.invoke_callback(callback_ref.callback, Vec::new())
        });
        self.session = Some(idle);
        result.map_err(|error| match error {
            mms::EvalError::Host(error) if error.kind == mms::HostErrorKind::StaleHandle => {
                DeferredCallbackError::StaleCallback
            }
            error => DeferredCallbackError::Evaluation(error.to_string()),
        })?;

        intents.retain_mut(|intent| match mode {
            DeferredCallbackMode::AudioOnly { beat_context } => match intent {
                IntentValue::AudioSchedulePlay {
                    beat_context: context,
                    ..
                }
                | IntentValue::OscillatorScheduleSetPitch {
                    beat_context: context,
                    ..
                } => {
                    *context = Some(beat_context);
                    true
                }
                _ => false,
            },
            DeferredCallbackMode::VisualOnly => !matches!(
                intent,
                IntentValue::AudioSchedulePlay { .. }
                    | IntentValue::OscillatorScheduleSetPitch { .. }
            ),
        });
        Ok(intents)
    }

    /// Drain callback invocations queued by Rx and run them against the live
    /// engine host. Script table and closure identity persist across calls.
    pub fn service_callbacks(
        &mut self,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        if self.is_closed() {
            return EvalOutput {
                intents: Vec::new(),
                errors: vec!["Mittens RuntimeSpec session is closed".into()],
            };
        }
        let invocations = {
            let mut queued = self.callback_invocations.lock().unwrap();
            std::mem::take(&mut *queued)
        };
        if invocations.is_empty() {
            return EvalOutput::default();
        }

        let idle = self.session.take().expect("checked above");
        let mut intents = Vec::new();
        let mut host = crate::scripting::host::MittensHost::new(world, emit, &mut intents)
            .with_rx(rx)
            .with_bindings(self.configured.bindings())
            .with_callback_invocations(Arc::clone(&self.callback_invocations))
            .with_callback_delivery_enabled(Arc::clone(&self.callback_delivery_enabled));
        if let Some(render_assets) = render_assets {
            host = host.with_render_assets(render_assets);
        }
        let (idle, errors) = idle.with_host(host, |session| {
            let mut errors = Vec::new();
            for invocation in invocations {
                if let Err(error) = session.invoke_callback_invocation(invocation) {
                    errors.push(error.to_string());
                }
            }
            errors
        });
        self.session = Some(idle);

        EvalOutput { intents, errors }
    }
}

impl Drop for RuntimeSpecSession {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Debug, Clone)]
pub struct LoadedMmsModule {
    pub named_exports: HashMap<String, Value>,
    pub sequence: Vec<MaterializedCE>,
    pub heap: HeapHandle,
    pub source_path: Option<String>,
}

impl LoadedMmsModule {
    pub fn named_export(&self, name: &str) -> Option<&Value> {
        self.named_exports.get(name)
    }
}

/// Synchronous wrapper around [`MeowMeowEvaluator`].
///
/// Spawns an evaluator thread, sends a script, drains all responses to
/// completion, and returns the collected [`EvalOutput`]. The thread is shut
/// down and joined before returning.
pub struct MeowMeowRunner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleFactoryEvalMode {
    Template,
    Live,
}

fn headers_to_value(headers: &[(String, String)]) -> Value {
    Value::Map(
        headers
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect(),
    )
}

pub(crate) fn event_arg_value(signal: &crate::engine::ecs::Signal) -> Value {
    match signal.event.as_ref() {
        Some(crate::engine::ecs::EventSignal::KeyDown(event))
        | Some(crate::engine::ecs::EventSignal::KeyUp(event))
        | Some(crate::engine::ecs::EventSignal::KeyPress(event)) => Value::Map(HashMap::from([
            (
                "code".to_string(),
                event
                    .code
                    .as_ref()
                    .map_or(Value::Null, |code| Value::String(code.clone())),
            ),
            ("key".to_string(), Value::String(event.key.clone())),
        ])),
        Some(crate::engine::ecs::EventSignal::FrameTick { dt_sec }) => {
            Value::Map(HashMap::from([(
                "dt_sec".to_string(),
                Value::Number(*dt_sec as f64),
            )]))
        }
        Some(crate::engine::ecs::EventSignal::GltfInitialized { gltf, uri }) => {
            Value::Map(HashMap::from([
                (
                    "gltf".to_string(),
                    Value::ComponentObject {
                        id: *gltf,
                        component_type: "GLTF".to_string(),
                    },
                ),
                ("uri".to_string(), Value::String(uri.clone())),
            ]))
        }
        Some(crate::engine::ecs::EventSignal::DataEvent { name, .. }) => {
            Value::String(name.clone())
        }
        Some(crate::engine::ecs::EventSignal::ToggleChanged { toggle, value }) => {
            Value::Map(HashMap::from([
                (
                    "toggle".into(),
                    Value::ComponentObject {
                        id: *toggle,
                        component_type: "Toggle".into(),
                    },
                ),
                ("value".into(), Value::Bool(*value)),
            ]))
        }
        Some(crate::engine::ecs::EventSignal::MountStarted { rider, mountable })
        | Some(crate::engine::ecs::EventSignal::MountEnded { rider, mountable }) => {
            Value::Map(HashMap::from([
                (
                    "rider".into(),
                    Value::ComponentObject {
                        id: *rider,
                        component_type: "Rider".into(),
                    },
                ),
                (
                    "mountable".into(),
                    Value::ComponentObject {
                        id: *mountable,
                        component_type: "Mountable".into(),
                    },
                ),
            ]))
        }
        Some(crate::engine::ecs::EventSignal::SliderChanged { slider, value })
        | Some(crate::engine::ecs::EventSignal::SliderCommitted { slider, value }) => {
            Value::Map(HashMap::from([
                (
                    "slider".into(),
                    Value::ComponentObject {
                        id: *slider,
                        component_type: "Slider".into(),
                    },
                ),
                ("value".into(), Value::Number(*value as f64)),
            ]))
        }
        Some(crate::engine::ecs::EventSignal::XrButtonDown {
            hand,
            control,
            value,
            ..
        })
        | Some(crate::engine::ecs::EventSignal::XrButtonUp {
            hand,
            control,
            value,
            ..
        })
        | Some(crate::engine::ecs::EventSignal::XrButtonChanged {
            hand,
            control,
            value,
            ..
        }) => Value::Map(HashMap::from([
            ("hand".to_string(), Value::String(format!("{hand:?}"))),
            ("control".to_string(), Value::String(format!("{control:?}"))),
            ("value".to_string(), Value::Number(*value as f64)),
        ])),
        Some(crate::engine::ecs::EventSignal::XrAxisChanged {
            hand,
            control,
            value,
            ..
        }) => Value::Map(HashMap::from([
            ("hand".to_string(), Value::String(format!("{hand:?}"))),
            ("control".to_string(), Value::String(format!("{control:?}"))),
            (
                "value".to_string(),
                Value::Array(vec![
                    Value::Number(value[0] as f64),
                    Value::Number(value[1] as f64),
                ]),
            ),
        ])),
        Some(crate::engine::ecs::EventSignal::TextInputChanged { text, caret, .. }) => {
            Value::Map(HashMap::from([
                ("text".to_string(), Value::String(text.clone())),
                ("caret".to_string(), Value::Number(*caret as f64)),
            ]))
        }
        Some(crate::engine::ecs::EventSignal::HttpRequest {
            request_id,
            method,
            path,
            query,
            url,
            headers,
            body_text,
            remote_addr,
        }) => Value::Map(HashMap::from([
            ("request_id".to_string(), Value::Number(*request_id as f64)),
            ("method".to_string(), Value::String(method.clone())),
            ("path".to_string(), Value::String(path.clone())),
            (
                "query".to_string(),
                query
                    .as_ref()
                    .map(|query| Value::String(query.clone()))
                    .unwrap_or(Value::Null),
            ),
            ("url".to_string(), Value::String(url.clone())),
            ("target".to_string(), Value::String(url.clone())),
            ("headers".to_string(), headers_to_value(headers)),
            ("body_text".to_string(), Value::String(body_text.clone())),
            (
                "remote_addr".to_string(),
                remote_addr
                    .as_ref()
                    .map(|addr| Value::String(addr.clone()))
                    .unwrap_or(Value::Null),
            ),
        ])),
        Some(crate::engine::ecs::EventSignal::HttpResponse {
            request_id,
            status,
            ok,
            headers,
            body_text,
            url,
        }) => Value::Map(HashMap::from([
            ("request_id".to_string(), Value::Number(*request_id as f64)),
            ("status".to_string(), Value::Number(*status as f64)),
            ("ok".to_string(), Value::Bool(*ok)),
            ("headers".to_string(), headers_to_value(headers)),
            ("body_text".to_string(), Value::String(body_text.clone())),
            ("url".to_string(), Value::String(url.clone())),
        ])),
        Some(crate::engine::ecs::EventSignal::HttpError {
            request_id,
            phase,
            message,
            url,
            bind_addr,
        }) => Value::Map(HashMap::from([
            (
                "request_id".to_string(),
                request_id
                    .map(|request_id| Value::Number(request_id as f64))
                    .unwrap_or(Value::Null),
            ),
            ("phase".to_string(), Value::String(phase.clone())),
            ("message".to_string(), Value::String(message.clone())),
            (
                "url".to_string(),
                url.as_ref()
                    .map(|url| Value::String(url.clone()))
                    .unwrap_or(Value::Null),
            ),
            (
                "bind_addr".to_string(),
                bind_addr
                    .as_ref()
                    .map(|bind_addr| Value::String(bind_addr.clone()))
                    .unwrap_or(Value::Null),
            ),
        ])),
        Some(crate::engine::ecs::EventSignal::XrEyeTrackingUpdated {
            combined_look,
            left_look,
            right_look,
            combined_openness,
        }) => Value::Map(HashMap::from([
            ("combined_look".into(), look_value(*combined_look)),
            ("left_look".into(), look_value(*left_look)),
            ("right_look".into(), look_value(*right_look)),
            (
                "combined_openness".into(),
                combined_openness
                    .map(|v| Value::Number(v as f64))
                    .unwrap_or(Value::Null),
            ),
        ])),
        Some(crate::engine::ecs::EventSignal::XrEyeTrackingHtcUpdated { left, right }) => {
            Value::Map(HashMap::from([
                ("left".into(), htc_eye_value(left)),
                ("right".into(), htc_eye_value(right)),
            ]))
        }
        _ => Value::Null,
    }
}

fn look_value(look: Option<[f32; 3]>) -> Value {
    look.map(|v| Value::Array(v.into_iter().map(|x| Value::Number(x as f64)).collect()))
        .unwrap_or(Value::Null)
}
fn htc_eye_value(eye: &crate::engine::ecs::system::xr_eye_tracking_system::HtcEye) -> Value {
    Value::Map(HashMap::from([
        ("look".into(), look_value(eye.look)),
        (
            "position".into(),
            eye.position
                .map(|v| Value::Array(v.into_iter().map(|x| Value::Number(x as f64)).collect()))
                .unwrap_or(Value::Null),
        ),
        (
            "openness".into(),
            eye.openness
                .map(|v| Value::Number(v as f64))
                .unwrap_or(Value::Null),
        ),
        (
            "wide".into(),
            eye.wide
                .map(|v| Value::Number(v as f64))
                .unwrap_or(Value::Null),
        ),
        (
            "squeeze".into(),
            eye.squeeze
                .map(|v| Value::Number(v as f64))
                .unwrap_or(Value::Null),
        ),
        (
            "pupil_diameter".into(),
            eye.pupil_diameter
                .map(|v| Value::Number(v as f64))
                .unwrap_or(Value::Null),
        ),
    ]))
}

impl MeowMeowRunner {
    /// Evaluate through `meow-meow-script` using the strict crate-owned
    /// Mittens `RuntimeSpec`, then service host effects against the live ECS.
    ///
    /// This is the cutover entrypoint for the first component-spawn slice. It
    /// intentionally does not fall back to the legacy evaluator.
    pub fn eval_with_runtime_spec(
        source: &str,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        Self::eval_with_runtime_spec_at_path(source, None, world, rx, render_assets, emit)
    }

    /// Like [`Self::eval_with_runtime_spec`], while retaining the source path
    /// needed for relative MMS imports.
    pub fn eval_with_runtime_spec_at_path(
        source: &str,
        source_path: Option<&str>,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        let configured = match crate::scripting::runtime_config::build_mittens_runtime() {
            Ok(configured) => configured,
            Err(error) => {
                return EvalOutput {
                    intents: Vec::new(),
                    errors: vec![format!("Mittens RuntimeSpec build failed: {error}")],
                };
            }
        };

        let mut intents = Vec::new();
        let mut host = crate::scripting::host::MittensHost::new(world, emit, &mut intents)
            .with_rx(rx)
            .with_bindings(configured.bindings());
        if let Some(render_assets) = render_assets {
            host = host.with_render_assets(render_assets);
        }

        let source_id = match source_path {
            Some(path) => match source_identity(path) {
                Ok(identity) => Some(identity),
                Err(error) => {
                    return EvalOutput {
                        intents,
                        errors: vec![error],
                    };
                }
            },
            None => None,
        };
        let result = configured
            .runtime()
            .session(host)
            .eval_with_source_id(source, source_id)
            .map_err(|error| error.to_string());

        match result {
            Ok(_) => EvalOutput {
                intents,
                errors: Vec::new(),
            },
            Err(message) => EvalOutput {
                intents,
                errors: vec![message],
            },
        }
    }

    /// Evaluate `source` without a live ECS world, collecting emitted intents
    /// and errors.
    ///
    /// This mode cannot allocate live `ComponentId`s during evaluation, so
    /// let-bound component expressions stay as `ComponentExpr` values rather
    /// than becoming live `ComponentObject` handles.
    ///
    /// Times out after 2 seconds if the evaluator stalls.
    pub fn eval(source: &str) -> EvalOutput {
        Self::eval_impl(source, None, Duration::from_secs(2))
    }

    /// Evaluate `source` with a caller-provided timeout.
    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput {
        Self::eval_impl(source, None, timeout)
    }

    /// Evaluate `source` knowing it came from `path` (enables relative imports).
    pub fn eval_with_path(source: &str, path: &str) -> EvalOutput {
        Self::eval_impl(source, Some(path), Duration::from_secs(2))
    }

    /// Read `path` from disk and evaluate it (enables relative imports).
    pub fn eval_file(path: &str) -> EvalOutput {
        Self::eval_file_with_timeout(path, Duration::from_secs(2))
    }

    /// Read `path` from disk and evaluate it (enables relative imports) with a caller-provided timeout.
    pub fn eval_file_with_timeout(path: &str, timeout: Duration) -> EvalOutput {
        match std::fs::read_to_string(path) {
            Ok(source) => Self::eval_impl(&source, Some(path), timeout),
            Err(e) => {
                let mut output = EvalOutput::default();
                output
                    .errors
                    .push(format!("cannot read file '{}': {}", path, e));
                output
            }
        }
    }

    pub fn load_module_source(
        source: &str,
        source_path: Option<&str>,
    ) -> Result<LoadedMmsModule, String> {
        let module = match eval_module_source(source, source_path)? {
            Value::Module {
                named,
                sequence,
                heap,
            } => Ok(LoadedMmsModule {
                named_exports: named,
                sequence,
                heap,
                source_path: source_path.map(|s| s.to_string()),
            }),
            other => Err(format!(
                "load_module_source: expected module result, got {:?}",
                other
            )),
        }?;
        Ok(module)
    }

    pub fn load_module_file(path: &str) -> Result<LoadedMmsModule, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read module '{}': {}", path, e))?;
        Self::load_module_source(&source, Some(path))
    }

    pub fn call_mms_module_fn(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        channels: Option<&mut crate::scripting::world_evaluator::EvalChannels>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<Value, String> {
        let Some(export) = module.named_export(name) else {
            return Err(format!("call_mms_module_fn: export '{}' not found", name));
        };
        if !matches!(export, Value::Function { .. }) {
            return Err(format!(
                "call_mms_module_fn: export '{}' is not a function",
                name
            ));
        }
        eval_mms_fn(export, args, channels, world_host, emit)
    }

    pub fn materialize_mms_module_component(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<MaterializedCE, String> {
        Self::materialize_mms_module_component_in_mode(
            module,
            name,
            args,
            world_host,
            emit,
            ModuleFactoryEvalMode::Template,
        )
    }

    pub fn materialize_mms_module_component_in_mode(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
        mode: ModuleFactoryEvalMode,
    ) -> Result<MaterializedCE, String> {
        match mode {
            ModuleFactoryEvalMode::Template => {}
            ModuleFactoryEvalMode::Live => {
                return Err(
                    "materialize_mms_module_component_in_mode: live mode does not return a stable MaterializedCE; use a spawn/instantiate helper instead".to_string()
                )
            }
        }
        let _ = world_host;
        let _ = emit;
        let value = Self::call_mms_module_fn(module, name, args, None, None, None)?;
        let Value::ComponentExpr(component_expr) = value else {
            return Err(format!(
                "materialize_mms_module_component: export '{}' did not return a component tree",
                name
            ));
        };
        Ok(*component_expr)
    }

    pub fn materialize_mms_module_component_from_file(
        path: &str,
        name: &str,
        args: Vec<Value>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<MaterializedCE, String> {
        let module = Self::load_module_file(path)?;
        Self::materialize_mms_module_component(&module, name, args, world_host, emit)
    }

    pub fn spawn_mms_module_component_uninitialized(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        Self::spawn_mms_module_component_uninitialized_with_assets(
            module, name, args, world, None, emit,
        )
    }

    pub fn spawn_mms_module_component_uninitialized_with_assets(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world: &mut World,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        Self::spawn_mms_module_component_value(
            module,
            name,
            args,
            None,
            world,
            render_assets,
            emit,
            false,
        )
    }

    pub fn spawn_mms_module_component_uninitialized_from_file(
        path: &str,
        name: &str,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = Self::load_module_file(path)?;
        Self::spawn_mms_module_component_uninitialized(&module, name, args, world, emit)
    }

    pub fn spawn_mms_module_component(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        parent: Option<ComponentId>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        Self::spawn_mms_module_component_value(module, name, args, parent, world, None, emit, true)
    }

    pub fn spawn_mms_module_component_from_file(
        path: &str,
        name: &str,
        args: Vec<Value>,
        parent: Option<ComponentId>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = Self::load_module_file(path)?;
        Self::spawn_mms_module_component(&module, name, args, parent, world, emit)
    }

    fn spawn_mms_module_component_value(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        parent: Option<ComponentId>,
        world: &mut World,
        mut render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
        initialize: bool,
    ) -> Result<ComponentId, String> {
        let value = Self::eval_mms_module_component_live(
            module,
            name,
            args,
            world,
            render_assets.as_deref_mut(),
            emit,
        )?;
        match value {
            Value::ComponentObject { id, .. } => {
                if let Some(p) = parent {
                    world
                        .add_child(p, id)
                        .map_err(|e| format!("attach live module component failed: {e}"))?;
                }
                if initialize {
                    let should_init = parent.map(|p| world.is_initialized(p)).unwrap_or(true);
                    if should_init {
                        world.init_component_tree(id, emit);
                    }
                }
                Ok(id)
            }
            Value::ComponentExpr(component_expr) => {
                if let Some(render_assets) = render_assets.as_deref_mut() {
                    crate::scripting::component_registry::with_live_render_assets(
                        render_assets,
                        || {
                            if initialize {
                                crate::scripting::component_registry::spawn_tree(
                                    &component_expr,
                                    parent,
                                    world,
                                    emit,
                                )
                            } else {
                                crate::scripting::component_registry::spawn_tree_uninitialized(
                                    &component_expr,
                                    world,
                                    emit,
                                )
                            }
                        },
                    )
                } else if initialize {
                    crate::scripting::component_registry::spawn_tree(
                        &component_expr,
                        parent,
                        world,
                        emit,
                    )
                } else {
                    crate::scripting::component_registry::spawn_tree_uninitialized(
                        &component_expr,
                        world,
                        emit,
                    )
                }
            }
            other => Err(format!(
                "spawn_mms_module_component: export '{}' did not return a component tree, got {:?}",
                name, other
            )),
        }
    }

    fn eval_mms_module_component_live(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world: &mut World,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> Result<Value, String> {
        if let Some(render_assets) = render_assets {
            crate::scripting::component_registry::with_live_render_assets(render_assets, || {
                Self::call_mms_module_fn(module, name, args, None, Some(world), Some(emit))
            })
        } else {
            Self::call_mms_module_fn(module, name, args, None, Some(world), Some(emit))
        }
    }

    /// Evaluate `source` with live world access.
    ///
    /// Handles two HostCall kinds during evaluation:
    /// - `Spawn`: spawns the component tree into `world` and returns the root `ComponentId`.
    ///   `let x = T {}` binds a `ComponentObject(id)` instead of a dead `ComponentExpr`.
    /// - `RegisterHandler`: installs an MMS function as a scoped signal handler in `rx`.
    ///   `on(obj, "Click", fn(e) { ... })` registers without blocking the evaluator.
    pub fn eval_with_world(
        source: &str,
        world: &mut World,
        rx: &mut RxWorld,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        Self::eval_with_world_at_path(source, None, world, rx, emit)
    }

    /// Like `eval_with_world`, but also records the source file path so
    /// `import` statements resolve relative to it.
    pub fn eval_with_world_at_path(
        source: &str,
        source_path: Option<&str>,
        world: &mut World,
        rx: &mut RxWorld,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        Self::eval_with_world_and_assets_at_path(source, source_path, world, rx, None, emit)
    }

    /// Evaluate `source` with live world + render-asset access.
    pub fn eval_with_world_and_assets(
        source: &str,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        Self::eval_with_world_and_assets_at_path(source, None, world, rx, Some(render_assets), emit)
    }

    /// Like `eval_with_world_and_assets`, but also records the source file path so
    /// `import` statements resolve relative to it.
    pub fn eval_with_world_and_assets_at_path(
        source: &str,
        source_path: Option<&str>,
        world: &mut World,
        rx: &mut RxWorld,
        render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        Self::eval_with_legacy_world_evaluator(source, source_path, world, rx, render_assets, emit)
    }

    /// Migration-only implementation retained for parity fixtures while the
    /// crate-owned evaluator becomes the ordinary runner.
    #[allow(dead_code)]
    fn eval_with_legacy_world_evaluator(
        source: &str,
        source_path: Option<&str>,
        world: &mut World,
        rx: &mut RxWorld,
        mut render_assets: Option<&mut RenderAssets>,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        let mut handle = MeowMeowEvaluator::spawn(64);
        handle
            .requests
            .push(EvalRequest::EvalScript {
                source: source.to_string(),
                source_path: source_path.map(|s| s.to_string()),
            })
            .expect("MeowMeowRunner: push EvalScript");
        handle
            .requests
            .push(EvalRequest::Shutdown)
            .expect("MeowMeowRunner: push Shutdown");

        let mut output = EvalOutput::default();
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            match handle.responses.pop() {
                Ok(EvalResponse::Intent(iv)) => output.intents.push(iv),
                Ok(EvalResponse::Error { message }) => output.errors.push(message),
                Ok(EvalResponse::ParsedOk { .. }) => {}
                Ok(EvalResponse::SnippetComplete { .. }) => {}
                Ok(EvalResponse::NavigationComplete { .. } | EvalResponse::ReplReset) => {}
                Ok(EvalResponse::ShutdownAck) => break,
                Ok(EvalResponse::HostCall { id, kind }) => {
                    let reply = match kind {
                        HostCallKind::Spawn(ce) => {
                            let result = if let Some(render_assets) = render_assets.as_deref_mut() {
                                crate::scripting::component_registry::with_live_render_assets(
                                    render_assets,
                                    || {
                                        crate::scripting::component_registry::spawn_tree(
                                            &ce, None, world, emit,
                                        )
                                    },
                                )
                            } else {
                                crate::scripting::component_registry::spawn_tree(
                                    &ce, None, world, emit,
                                )
                            };
                            match result {
                                Ok(component_id) => HostValue::ComponentId(component_id),
                                Err(e) => {
                                    output.errors.push(format!("HostCall::Spawn error: {e}"));
                                    HostValue::Null
                                }
                            }
                        }
                        HostCallKind::Register(ce) => {
                            let result = if let Some(render_assets) = render_assets.as_deref_mut() {
                                crate::scripting::component_registry::with_live_render_assets(
                                    render_assets,
                                    || {
                                        crate::scripting::component_registry::spawn_tree_uninitialized(
                                            &ce, world, emit,
                                        )
                                    },
                                )
                            } else {
                                crate::scripting::component_registry::spawn_tree_uninitialized(
                                    &ce, world, emit,
                                )
                            };
                            match result {
                                Ok(component_id) => HostValue::ComponentId(component_id),
                                Err(e) => {
                                    output.errors.push(format!("HostCall::Register error: {e}"));
                                    HostValue::Null
                                }
                            }
                        }
                        HostCallKind::Attach { parent, child } => {
                            if let Some(p) = parent {
                                if let Err(e) = world.add_child(p, child) {
                                    output.errors.push(format!("HostCall::Attach error: {e}"));
                                }
                            }
                            // Run the deferred init walk on the (now-attached, or root) subtree.
                            world.init_component_tree(child, emit);
                            HostValue::Null
                        }
                        HostCallKind::Query {
                            selector,
                            scope,
                            multiple,
                        } => {
                            let roots: Vec<crate::engine::ecs::ComponentId> = match scope {
                                Some(id) => world.scripting_query_roots(id),
                                None => world
                                    .all_components()
                                    .filter(|&id| world.parent_of(id).is_none())
                                    .collect(),
                            };
                            let mut all_ids: Vec<crate::engine::ecs::ComponentId> = Vec::new();
                            for r in roots {
                                if multiple {
                                    all_ids.extend(world.find_all_components(r, &selector));
                                } else if let Some(found) = world.find_component(r, &selector) {
                                    all_ids.push(found);
                                    break;
                                }
                            }
                            if multiple {
                                let list = all_ids
                                    .into_iter()
                                    .filter_map(|id| {
                                        world.component_name(id).map(|t| (id, t.to_string()))
                                    })
                                    .collect();
                                HostValue::ComponentList(list)
                            } else {
                                match all_ids.into_iter().next() {
                                    Some(id) => match world.component_name(id) {
                                        Some(t) => HostValue::Component {
                                            id,
                                            component_type: t.to_string(),
                                        },
                                        None => HostValue::Null,
                                    },
                                    None => HostValue::Null,
                                }
                            }
                        }
                        HostCallKind::RegisterHandler {
                            scope,
                            signal_kind,
                            name,
                            handler,
                        } => {
                            let callback =
                                move |world: &mut World,
                                      emit: &mut dyn SignalEmitter,
                                      signal: &crate::engine::ecs::Signal| {
                                    let arg = event_arg_value(signal);
                                    if let Err(e) = eval_mms_fn(
                                        &handler,
                                        vec![arg],
                                        None,
                                        Some(world),
                                        Some(emit),
                                    ) {
                                        eprintln!("[mms] handler error: {e}");
                                    }
                                };
                            if let Some(name) = name {
                                rx.add_handler_closure_named(
                                    signal_kind,
                                    scope,
                                    Some(name),
                                    callback,
                                );
                            } else {
                                rx.add_handler_closure(signal_kind, scope, callback);
                            }
                            HostValue::Null
                        }
                        HostCallKind::RegisterGlobalHandler {
                            signal_kind,
                            name,
                            handler,
                        } => {
                            let callback = move |world: &mut World,
                                                 emit: &mut dyn SignalEmitter,
                                                 signal: &crate::engine::ecs::Signal| {
                                let arg = event_arg_value(signal);
                                if let Err(e) = eval_mms_fn(
                                    &handler,
                                    vec![arg],
                                    None,
                                    Some(world),
                                    Some(emit),
                                ) {
                                    eprintln!("[mms] global handler error: {e}");
                                }
                            };
                            if let Some(name) = name {
                                rx.add_global_handler_closure_named(
                                    signal_kind,
                                    Some(name),
                                    callback,
                                );
                            } else {
                                rx.add_global_handler_closure(signal_kind, callback);
                            }
                            HostValue::Null
                        }
                        HostCallKind::AudioClipInstance {
                            source,
                            start_beat,
                            stop_beat,
                        } => {
                            use crate::engine::ecs::component::AudioClipComponent;
                            match world.get_component_by_id_as::<AudioClipComponent>(source) {
                                Some(src) => {
                                    let mut c = AudioClipComponent::instance_of(src);
                                    if let Some(sb) = start_beat {
                                        c.start_beat = sb;
                                    }
                                    if let Some(eb) = stop_beat {
                                        c.stop_beat = Some(eb);
                                    }
                                    let id = world.add_component(c);
                                    HostValue::ComponentId(id)
                                }
                                None => {
                                    output.errors.push(
                                        "HostCall::AudioClipInstance: source is not an AudioClip"
                                            .to_string(),
                                    );
                                    HostValue::Null
                                }
                            }
                        }
                        HostCallKind::InvokeComponentMethod {
                            id,
                            component_type,
                            method,
                            args,
                        } => match crate::scripting::component_method_registry::invoke_component_method(
                            world,
                            id,
                            &component_type,
                            &method,
                            &args,
                            |intent| output.intents.push(intent),
                        ) {
                            Ok(value) => match value {
                                Value::Null => HostValue::Null,
                                Value::ComponentObject { id, component_type } => {
                                    HostValue::Component { id, component_type }
                                }
                                other => HostValue::Value(other),
                            },
                            Err(e) => {
                                output
                                    .errors
                                    .push(format!("HostCall::InvokeComponentMethod error: {e}"));
                                HostValue::Null
                            }
                        },
                        HostCallKind::ReplTree { .. }
                        | HostCallKind::ReplDump { .. }
                        | HostCallKind::ReplHelp
                        | HostCallKind::ReplClear => HostValue::Null,
                    };
                    let _ = handle
                        .requests
                        .push(EvalRequest::HostCallResult { id, value: reply });
                }
                Err(rtrb::PopError::Empty) => {
                    if Instant::now() > deadline {
                        output
                            .errors
                            .push("MeowMeowRunner: timed out waiting for evaluator".into());
                        break;
                    }
                    std::thread::yield_now();
                }
            }
        }

        handle.shutdown_and_join();
        output
    }

    fn eval_impl(source: &str, source_path: Option<&str>, timeout: Duration) -> EvalOutput {
        let mut handle = MeowMeowEvaluator::spawn(64);

        handle
            .requests
            .push(EvalRequest::EvalScript {
                source: source.to_string(),
                source_path: source_path.map(|s| s.to_string()),
            })
            .expect("MeowMeowRunner: push EvalScript");
        handle
            .requests
            .push(EvalRequest::Shutdown)
            .expect("MeowMeowRunner: push Shutdown");

        let mut output = EvalOutput::default();
        let deadline = Instant::now() + timeout;

        loop {
            match handle.responses.pop() {
                Ok(EvalResponse::Intent(iv)) => output.intents.push(iv),
                Ok(EvalResponse::Error { message }) => output.errors.push(message),
                Ok(EvalResponse::ParsedOk { .. }) => {}
                Ok(EvalResponse::SnippetComplete { .. }) => {}
                Ok(EvalResponse::NavigationComplete { .. } | EvalResponse::ReplReset) => {}
                Ok(EvalResponse::ShutdownAck) => break,
                // Fire-and-forget runner has no world — reply null so the evaluator
                // falls back to ComponentExpr and continues without blocking.
                Ok(EvalResponse::HostCall { id, .. }) => {
                    let _ = handle.requests.push(EvalRequest::HostCallResult {
                        id,
                        value: HostValue::Null,
                    });
                }
                Err(rtrb::PopError::Empty) => {
                    if Instant::now() > deadline {
                        output
                            .errors
                            .push("MeowMeowRunner: timed out waiting for evaluator".into());
                        break;
                    }
                    std::thread::yield_now();
                }
            }
        }

        handle.shutdown_and_join();
        output
    }
}

fn source_identity(path: &str) -> Result<mms::SourceId, String> {
    let path = std::path::Path::new(path);
    let resolved = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let parent = parent.canonicalize().map_err(|error| {
                format!("cannot resolve source path '{}': {error}", path.display())
            })?;
            let file_name = path
                .file_name()
                .ok_or_else(|| format!("source path '{}' has no file name", path.display()))?;
            parent.join(file_name)
        }
    };
    Ok(mms::SourceId::new(resolved.to_string_lossy()))
}

#[cfg(test)]
mod runtime_spec_session_tests {
    use super::*;
    use crate::engine::ecs::component::EmissiveComponent;
    use crate::engine::ecs::{CommandQueue, EventSignal, Signal};

    #[test]
    fn keyboard_payload_has_exactly_code_and_key_fields() {
        let signal = Signal::event(
            ComponentId::default(),
            EventSignal::KeyDown(crate::engine::ecs::KeyboardEvent {
                code: Some("KeyW".to_string()),
                key: "W".to_string(),
            }),
        );
        let Value::Map(payload) = event_arg_value(&signal) else {
            panic!("keyboard callback payload must be a table");
        };
        assert_eq!(payload.len(), 2);
        assert_eq!(
            payload.get("code"),
            Some(&Value::String("KeyW".to_string()))
        );
        assert_eq!(payload.get("key"), Some(&Value::String("W".to_string())));
    }

    #[test]
    fn keyboard_events_example_evaluates_and_registers_all_global_routes() {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut commands = CommandQueue::new();
        let (_session, _intents) = RuntimeSpecSession::start(
            include_str!("../../examples/keyboard-events.mms"),
            &mut world,
            &mut rx,
            None,
            &mut commands,
        )
        .expect("keyboard example should evaluate");
        assert!(rx.has_global_handlers(crate::engine::ecs::SignalKind::KeyDown));
        assert!(rx.has_global_handlers(crate::engine::ecs::SignalKind::KeyPress));
        assert!(rx.has_global_handlers(crate::engine::ecs::SignalKind::KeyUp));
    }

    #[test]
    fn runtime_spec_session_supports_math_constants_as_properties() {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut commands = CommandQueue::new();
        let source = r#"
            Transform.rotation(0.0, Math.pi, 0.0) { name = "pi-rotation" }
        "#;

        let (_session, _intents) =
            RuntimeSpecSession::start(source, &mut world, &mut rx, None, &mut commands).unwrap();
        assert!(
            world
                .all_components()
                .any(|id| world.component_label(id) == Some("pi-rotation"))
        );
    }

    #[test]
    fn queued_click_callbacks_toggle_emissive_and_retain_table_state() {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut commands = CommandQueue::new();
        let source = r#"
            let glow = Emissive.on() { name = "glow" intensity(3.0) }
            let cube = Transform { name = "cube" glow }
            let state = { a = 0.0 }
            on(cube, "Click", fn(event) {
                if state.a == 0.0 {
                    state.a = 1.0
                    glow.set_intensity(0.15)
                } else {
                    state.a = 0.0
                    glow.set_intensity(3.0)
                }
            })
            cube
        "#;
        let (mut session, _) =
            RuntimeSpecSession::start(source, &mut world, &mut rx, None, &mut commands).unwrap();
        let labelled = |world: &World, label: &str| {
            world
                .all_components()
                .find(|&id| world.component_label(id) == Some(label))
                .unwrap()
        };
        let cube = labelled(&world, "cube");
        let glow = labelled(&world, "glow");
        let click = || {
            Signal::event(
                cube,
                EventSignal::Click {
                    raycaster: ComponentId::default(),
                    renderable: cube,
                    hit_point: [0.0; 3],
                    screen_pos_px: None,
                },
            )
        };

        rx.dispatch_event_handlers(&mut world, &click());
        let output = session.service_callbacks(&mut world, &mut rx, None, &mut commands);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        assert_eq!(
            world
                .get_component_by_id_as::<EmissiveComponent>(glow)
                .unwrap()
                .intensity,
            0.15
        );

        rx.dispatch_event_handlers(&mut world, &click());
        let output = session.service_callbacks(&mut world, &mut rx, None, &mut commands);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        assert_eq!(
            world
                .get_component_by_id_as::<EmissiveComponent>(glow)
                .unwrap()
                .intensity,
            3.0
        );
    }

    #[test]
    fn retained_runtime_transform_info_panel_updates_text_on_frame_tick() {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut commands = CommandQueue::new();
        let source = r##"
            import { transform_info_panel } from "../assets/components/ui/transform_info_panel.mms"

            let target = Transform.position(-1.234567, 0.0, 10.5) { name = "telemetry_target" }
            target
            transform_info_panel(target)
        "##;
        let (mut session, output) = RuntimeSpecSession::start_at_path(
            source,
            "examples/_mms_test_transform_info_panel_runtime_spec.mms",
            &mut world,
            &mut rx,
            None,
            &mut commands,
        )
        .unwrap();
        assert!(output.errors.is_empty(), "{:?}", output.errors);

        rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                ComponentId::default(),
                EventSignal::FrameTick { dt_sec: 0.1 },
            ),
        );
        let output = session.service_callbacks(&mut world, &mut rx, None, &mut commands);

        assert!(output.errors.is_empty(), "{:?}", output.errors);
        let texts: Vec<_> = output
            .intents
            .into_iter()
            .filter_map(|intent| match intent {
                IntentValue::SetText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["x: -1.23457", "y: 0.00000", "z: 10.50000"]);
    }

    #[test]
    fn configured_runtime_sessions_are_isolated_and_close_disables_callback_delivery() {
        let configured =
            Arc::new(crate::scripting::runtime_config::build_mittens_runtime().unwrap());
        let source = r#"
            let root = Transform { name = "session-one" }
            on(root, "Click", fn(event) {})
            root
        "#;
        let mut first_world = World::default();
        let mut first_rx = RxWorld::default();
        let mut first_commands = CommandQueue::new();
        let (mut first, output) = RuntimeSpecSession::start_with_runtime(
            configured.clone(),
            source,
            None,
            &mut first_world,
            &mut first_rx,
            None,
            &mut first_commands,
        )
        .unwrap();
        assert!(output.errors.is_empty());

        let mut second_world = World::default();
        let mut second_rx = RxWorld::default();
        let mut second_commands = CommandQueue::new();
        let (mut second, output) = RuntimeSpecSession::start_with_runtime(
            configured,
            "Transform { name = \"session-two\" }",
            None,
            &mut second_world,
            &mut second_rx,
            None,
            &mut second_commands,
        )
        .unwrap();
        assert!(output.errors.is_empty());

        let root = first_world
            .all_components()
            .find(|&id| first_world.component_label(id) == Some("session-one"))
            .unwrap();
        first_rx.dispatch_event_handlers(
            &mut first_world,
            &Signal::event(
                root,
                EventSignal::Click {
                    raycaster: ComponentId::default(),
                    renderable: root,
                    hit_point: [0.0; 3],
                    screen_pos_px: None,
                },
            ),
        );
        let invocation = first.callback_invocations.lock().unwrap().pop().unwrap();
        second.callback_invocations.lock().unwrap().push(invocation);
        let output = second.service_callbacks(
            &mut second_world,
            &mut second_rx,
            None,
            &mut second_commands,
        );
        assert_eq!(output.errors.len(), 1);
        assert!(output.errors[0].contains("belongs to another session"));

        first.close();
        assert!(first.is_closed());
        first_rx.dispatch_event_handlers(
            &mut first_world,
            &Signal::event(
                root,
                EventSignal::Click {
                    raycaster: ComponentId::default(),
                    renderable: root,
                    hit_point: [0.0; 3],
                    screen_pos_px: None,
                },
            ),
        );
        assert!(first.callback_invocations.lock().unwrap().is_empty());
        let output =
            first.service_callbacks(&mut first_world, &mut first_rx, None, &mut first_commands);
        assert_eq!(output.errors, vec!["Mittens RuntimeSpec session is closed"]);
    }
}
