use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use meow_meow_script as mms;
use slotmap::{Key, KeyData};

use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, SignalKind, World};
use crate::engine::graphics::RenderAssets;
use crate::scripting::object as legacy;

/// A resolved engine signal route whose callback remains owned by an MMS
/// session. The session driver is responsible for retaining the session and
/// invoking the callback asynchronously when the route fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalCallbackRoute {
    pub kind: SignalKind,
    pub scope: Option<ComponentId>,
    pub name: Option<String>,
    pub callback: mms::CallbackHandle,
}

/// Capability lease applied while a retained keyframe callback is evaluated.
/// The evaluator may still inspect its session-owned closure, but a lease
/// prevents host-visible work from crossing into the wrong animation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredCallbackHostPhase {
    Audio,
    Visual,
}

/// Engine implementation of the host-neutral Meow Meow host contract.
pub(crate) struct MittensHost<'a, 'bindings> {
    pub world: &'a mut World,
    pub rx: Option<&'a mut RxWorld>,
    pub render_assets: Option<&'a mut RenderAssets>,
    pub emit: &'a mut dyn SignalEmitter,
    pub intents: &'a mut Vec<IntentValue>,
    bindings: Option<&'bindings mms::ImplementationBindings<super::runtime_config::MittensBinding>>,
    signal_routes: Option<&'a mut Vec<SignalCallbackRoute>>,
    callback_invocations: Option<Arc<Mutex<Vec<mms::CallbackInvocation>>>>,
    callback_delivery_enabled: Option<Arc<AtomicBool>>,
    deferred_callback_phase: Option<DeferredCallbackHostPhase>,
    legacy_component_fallbacks: usize,
    legacy_method_fallbacks: usize,
}

impl<'a, 'bindings> MittensHost<'a, 'bindings> {
    /// Assemble the standard live-engine lease used by RuntimeSpec work.
    /// Keeping this bundle here prevents session and application layers from
    /// learning which Mittens services each host operation borrows.
    pub(crate) fn runtime_lease(
        world: &'a mut World,
        rx: &'a mut RxWorld,
        render_assets: &'a mut RenderAssets,
        emit: &'a mut dyn SignalEmitter,
        intents: &'a mut Vec<IntentValue>,
    ) -> MittensHost<'a, 'a> {
        MittensHost::new(world, emit, intents)
            .with_rx(rx)
            .with_render_assets(render_assets)
    }

    pub fn new(
        world: &'a mut World,
        emit: &'a mut dyn SignalEmitter,
        intents: &'a mut Vec<IntentValue>,
    ) -> Self {
        Self {
            world,
            rx: None,
            render_assets: None,
            emit,
            intents,
            bindings: None,
            signal_routes: None,
            callback_invocations: None,
            callback_delivery_enabled: None,
            deferred_callback_phase: None,
            legacy_component_fallbacks: 0,
            legacy_method_fallbacks: 0,
        }
    }

    pub fn with_rx(mut self, rx: &'a mut RxWorld) -> Self {
        self.rx = Some(rx);
        self
    }
    pub fn with_render_assets(mut self, assets: &'a mut RenderAssets) -> Self {
        self.render_assets = Some(assets);
        self
    }

    pub fn with_bindings<'next>(
        self,
        bindings: &'next mms::ImplementationBindings<super::runtime_config::MittensBinding>,
    ) -> MittensHost<'a, 'next> {
        MittensHost {
            world: self.world,
            rx: self.rx,
            render_assets: self.render_assets,
            emit: self.emit,
            intents: self.intents,
            bindings: Some(bindings),
            signal_routes: self.signal_routes,
            callback_invocations: self.callback_invocations,
            callback_delivery_enabled: self.callback_delivery_enabled,
            deferred_callback_phase: self.deferred_callback_phase,
            legacy_component_fallbacks: self.legacy_component_fallbacks,
            legacy_method_fallbacks: self.legacy_method_fallbacks,
        }
    }

    pub fn with_signal_routes(mut self, routes: &'a mut Vec<SignalCallbackRoute>) -> Self {
        self.signal_routes = Some(routes);
        self
    }

    pub fn signal_routes(&self) -> Option<&[SignalCallbackRoute]> {
        self.signal_routes.as_deref().map(Vec::as_slice)
    }

    pub fn with_callback_invocations(
        mut self,
        invocations: Arc<Mutex<Vec<mms::CallbackInvocation>>>,
    ) -> Self {
        self.callback_invocations = Some(invocations);
        self
    }

    /// Gate callback delivery for the lifetime of the session that owns the
    /// queued invocations. Closing that session disables its Rx routes even if
    /// the engine has not yet removed their host-side registrations.
    pub fn with_callback_delivery_enabled(mut self, enabled: Arc<AtomicBool>) -> Self {
        self.callback_delivery_enabled = Some(enabled);
        self
    }

    pub fn with_deferred_callback_phase(mut self, phase: DeferredCallbackHostPhase) -> Self {
        self.deferred_callback_phase = Some(phase);
        self
    }

    fn phase_blocks_tree(&self, tree: &mms::MaterializedCE) -> bool {
        matches!(
            self.deferred_callback_phase,
            Some(DeferredCallbackHostPhase::Audio)
        ) && tree.component_type != "MusicNote"
            || matches!(
                self.deferred_callback_phase,
                Some(DeferredCallbackHostPhase::Visual)
            ) && tree.component_type == "MusicNote"
    }

    fn inert_component(component_type: String) -> mms::HostResponse {
        // The value only lets evaluation continue past a deliberately gated
        // expression. It is never registered with the world, so any later
        // attempt to use it is correctly rejected as stale.
        mms::HostResponse::Component {
            handle: mms::ComponentHandle::from_raw(u64::MAX),
            component_type,
        }
    }

    pub fn legacy_component_fallbacks(&self) -> usize {
        self.legacy_component_fallbacks
    }

    pub fn legacy_method_fallbacks(&self) -> usize {
        self.legacy_method_fallbacks
    }

    /// Dispatch one engine event through the configured Rx routes. Script
    /// handlers enqueue opaque callback invocations; they never re-enter MMS
    /// from inside Rx dispatch.
    pub fn dispatch_event_handlers(
        &mut self,
        signal: &crate::engine::ecs::Signal,
    ) -> Result<(), mms::HostError> {
        let Some(rx) = self.rx.as_deref_mut() else {
            return Err(mms::HostError::unavailable(
                "register_signal_handler",
                "the current Mittens host lease has no signal routing service",
            ));
        };
        rx.dispatch_event_handlers(self.world, signal);
        Ok(())
    }

    pub fn component_handle(id: ComponentId) -> mms::ComponentHandle {
        mms::ComponentHandle::from_raw(id.data().as_ffi())
    }

    pub fn component_id(handle: mms::ComponentHandle) -> ComponentId {
        ComponentId::from(KeyData::from_ffi(handle.into_raw()))
    }

    fn existing_id(
        &self,
        handle: mms::ComponentHandle,
        operation: &str,
    ) -> Result<ComponentId, mms::HostError> {
        let id = Self::component_id(handle);
        self.world
            .get_component_record(id)
            .map(|_| id)
            .ok_or_else(|| {
                mms::HostError::failure(
                    operation,
                    format!("component handle {handle:?} is stale or foreign"),
                )
            })
    }

    fn record_signal_route(
        &mut self,
        signal: &str,
        scope: Option<mms::ComponentHandle>,
        name: Option<String>,
        callback: mms::CallbackHandle,
    ) -> Result<mms::HostResponse, mms::HostError> {
        let kind = signal_kind(signal).ok_or_else(|| {
            mms::HostError::failure(
                "register_signal_handler",
                format!("unknown signal '{signal}'"),
            )
        })?;
        let scope = scope
            .map(|scope| self.existing_id(scope, "register_signal_handler"))
            .transpose()?;
        if self.signal_routes.is_none()
            && (self.rx.is_none() || self.callback_invocations.is_none())
        {
            return Err(mms::HostError::unavailable(
                "register_signal_handler",
                "the current Mittens host lease has no callback queue or Rx service",
            ));
        }
        if let Some(routes) = self.signal_routes.as_deref_mut() {
            routes.push(SignalCallbackRoute {
                kind,
                scope,
                name: name.clone(),
                callback,
            });
        }
        if let (Some(rx), Some(invocations)) =
            (self.rx.as_deref_mut(), self.callback_invocations.as_ref())
        {
            let invocations = Arc::clone(invocations);
            let delivery_enabled = self.callback_delivery_enabled.as_ref().map(Arc::clone);
            let enqueue = move |_world: &mut World,
                                _emit: &mut dyn SignalEmitter,
                                signal: &crate::engine::ecs::Signal| {
                if delivery_enabled
                    .as_ref()
                    .is_some_and(|enabled| !enabled.load(Ordering::Acquire))
                {
                    return;
                }
                match event_arg_transport(signal) {
                    Ok(argument) => invocations.lock().unwrap().push(mms::CallbackInvocation {
                        callback,
                        args: vec![argument],
                    }),
                    Err(error) => eprintln!("[mms] signal payload conversion error: {error}"),
                }
            };
            if let Some(scope) = scope {
                rx.add_handler_closure_named(kind, scope, name, enqueue);
            } else {
                rx.add_global_handler_closure_named(kind, name, enqueue);
            }
        }
        Ok(mms::HostResponse::Unit)
    }
}

impl mms::Host for MittensHost<'_, '_> {
    fn dispatch_with_context(
        &mut self,
        context: &mut mms::HostContext,
        request: mms::HostRequest,
    ) -> Result<mms::HostResponse, mms::HostError> {
        let callback = match &request {
            mms::HostRequest::RegisterSignalHandler { callback, .. }
            | mms::HostRequest::RegisterSignalHandlerByName { callback, .. } => Some(*callback),
            _ => None,
        };
        if let Some(callback) = callback
            && !context.owns_callback(callback)
        {
            return Err(mms::HostError {
                kind: mms::HostErrorKind::ForeignHandle,
                operation: request.operation_name().into(),
                message: format!("callback handle {callback:?} is stale or foreign"),
            });
        }
        self.dispatch(request)
    }

    fn dispatch(&mut self, request: mms::HostRequest) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostRequest as R, HostResponse as S};
        match request {
            R::Emit { tree } => {
                let component_type = tree.component_type.clone();
                let response = self.dispatch(R::Spawn { tree })?;
                let S::Component { handle: native, .. } = response else {
                    return Err(mms::HostError::failure(
                        "emit",
                        "spawn did not return a component",
                    ));
                };
                Ok(S::Component {
                    handle: native,
                    component_type,
                })
            }
            R::RegisterComponent { tree } => {
                let component_type = tree.component_type.clone();
                let response = self.dispatch(R::Register { tree })?;
                let S::Component { handle: native, .. } = response else {
                    return Err(mms::HostError::failure(
                        "register_component",
                        "registration did not return a component",
                    ));
                };
                Ok(S::Component {
                    handle: native,
                    component_type,
                })
            }
            R::CallApi { api_id, .. } => Err(mms::HostError::unsupported(api_id)),
            R::CallApiById { operation_id, args } => {
                if self.deferred_callback_phase == Some(DeferredCallbackHostPhase::Audio) {
                    return Ok(S::Unit);
                }
                let Some(binding) = self
                    .bindings
                    .and_then(|bindings| bindings.get(operation_id))
                else {
                    return Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: "operation ID is not present in the Mittens binding table".into(),
                    });
                };
                match binding {
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::Smoke,
                    ) if args.is_empty() => Ok(S::Unit),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::Smoke,
                    ) => Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: "mittens.smoke expects no arguments".into(),
                    }),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::FileReadText,
                    ) => file_read_text(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::JsonParse,
                    ) => json_parse(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::JsonStringify,
                    ) => json_stringify(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::AudioInputDevices,
                    ) => audio_input_devices(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::AudioOutputDevices,
                    ) => audio_output_devices(args),
                    binding => Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: format!("{binding:?} cannot be invoked as an API"),
                    }),
                }
            }
            R::Spawn { tree } => {
                if self.phase_blocks_tree(&tree) {
                    return Ok(Self::inert_component(tree.component_type));
                }
                if let Some(bindings) = self.bindings {
                    if let Some(result) = super::configured_registry::try_spawn_tree(
                        &tree, bindings, self.world, self.emit, true,
                    ) {
                        let component_type = tree.component_type.clone();
                        let id = result.map_err(|error| mms::HostError::failure("spawn", error))?;
                        return Ok(S::Component {
                            handle: Self::component_handle(id),
                            component_type,
                        });
                    }
                }
                reject_animation_legacy_fallback(&tree, "spawn")?;
                self.legacy_component_fallbacks += 1;
                let tree = external_tree_to_legacy(tree)?;
                let result = if let Some(assets) = self.render_assets.as_deref_mut() {
                    crate::scripting::component_registry::with_live_render_assets(assets, || {
                        crate::scripting::component_registry::spawn_tree(
                            &tree, None, self.world, self.emit,
                        )
                    })
                } else {
                    crate::scripting::component_registry::spawn_tree(
                        &tree, None, self.world, self.emit,
                    )
                };
                let id = result.map_err(|e| mms::HostError::failure("spawn", e))?;
                Ok(S::Component {
                    handle: Self::component_handle(id),
                    component_type: tree.component_type,
                })
            }
            R::Register { tree } => {
                if self.phase_blocks_tree(&tree) {
                    return Ok(Self::inert_component(tree.component_type));
                }
                if let Some(bindings) = self.bindings {
                    if let Some(result) = super::configured_registry::try_spawn_tree(
                        &tree, bindings, self.world, self.emit, false,
                    ) {
                        let component_type = tree.component_type.clone();
                        let id =
                            result.map_err(|error| mms::HostError::failure("register", error))?;
                        return Ok(S::Component {
                            handle: Self::component_handle(id),
                            component_type,
                        });
                    }
                }
                reject_animation_legacy_fallback(&tree, "register")?;
                self.legacy_component_fallbacks += 1;
                let tree = external_tree_to_legacy(tree)?;
                let result = if let Some(assets) = self.render_assets.as_deref_mut() {
                    crate::scripting::component_registry::with_live_render_assets(assets, || {
                        crate::scripting::component_registry::spawn_tree_uninitialized(
                            &tree, self.world, self.emit,
                        )
                    })
                } else {
                    crate::scripting::component_registry::spawn_tree_uninitialized(
                        &tree, self.world, self.emit,
                    )
                };
                let id = result.map_err(|e| mms::HostError::failure("register", e))?;
                Ok(S::Component {
                    handle: Self::component_handle(id),
                    component_type: tree.component_type,
                })
            }
            R::Attach { parent, child } => {
                if self.deferred_callback_phase == Some(DeferredCallbackHostPhase::Audio) {
                    return Ok(S::Unit);
                }
                let child = self.existing_id(child, "attach")?;
                if let Some(parent) = parent {
                    let parent = self.existing_id(parent, "attach")?;
                    self.world
                        .add_child(parent, child)
                        .map_err(|e| mms::HostError::failure("attach", e))?;
                }
                self.world.init_component_tree(child, self.emit);
                Ok(S::Unit)
            }
            R::Query {
                selector,
                scope,
                multiple,
            } => {
                let roots = if let Some(scope) = scope {
                    self.world
                        .scripting_query_roots(self.existing_id(scope, "query")?)
                } else {
                    self.world
                        .all_components()
                        .filter(|&id| self.world.parent_of(id).is_none())
                        .collect()
                };
                let mut matches = Vec::new();
                for root in roots {
                    if multiple {
                        matches.extend(self.world.find_all_components(root, &selector));
                    } else if let Some(id) = self.world.find_component(root, &selector) {
                        matches.push(id);
                        break;
                    }
                }
                if multiple {
                    Ok(S::Components(
                        matches
                            .into_iter()
                            .filter_map(|id| {
                                self.world
                                    .component_name(id)
                                    .map(|ty| (Self::component_handle(id), ty.to_owned()))
                            })
                            .collect(),
                    ))
                } else if let Some(id) = matches.into_iter().next() {
                    let component_type = self
                        .world
                        .component_name(id)
                        .unwrap_or("Component")
                        .to_owned();
                    Ok(S::Component {
                        handle: Self::component_handle(id),
                        component_type,
                    })
                } else {
                    Ok(S::Unit)
                }
            }
            R::InvokeComponentMethod {
                operation_id,
                component,
                args,
            } => {
                let Some(binding) = self
                    .bindings
                    .and_then(|bindings| bindings.get(operation_id))
                else {
                    return Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: "component method ID is not present in the Mittens binding table"
                            .into(),
                    });
                };
                let super::runtime_config::MittensBinding::ComponentMethod {
                    component: component_type,
                    name: method,
                } = binding
                else {
                    return Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: format!("{binding:?} cannot be invoked as a component method"),
                    });
                };
                let is_audio = matches!(
                    *component_type,
                    "AudioOscillator" | "AudioClip" | "AudioGain"
                );
                if matches!(self.deferred_callback_phase, Some(DeferredCallbackHostPhase::Audio) if !is_audio)
                    || matches!(self.deferred_callback_phase, Some(DeferredCallbackHostPhase::Visual) if is_audio)
                {
                    return Ok(S::Value(mms::Value::Null));
                }
                let id = self.existing_id(component, "invoke_component_method")?;
                let args = args
                    .into_iter()
                    .map(external_value_to_legacy)
                    .collect::<Result<Vec<_>, _>>()?;
                let value = crate::scripting::component_method_registry::invoke_component_method(
                    self.world,
                    id,
                    component_type,
                    method,
                    &args,
                    |intent| self.intents.push(intent),
                )
                .map_err(|e| mms::HostError::failure(format!("{operation_id:?}"), e))?;
                Ok(S::Value(legacy_value_to_external(value)?))
            }
            R::InvokeComponentMethodByName {
                component,
                component_type,
                method,
                args,
            } => {
                let is_audio = matches!(
                    component_type.as_str(),
                    "AudioOscillator" | "AudioClip" | "AudioGain"
                );
                if matches!(self.deferred_callback_phase, Some(DeferredCallbackHostPhase::Audio) if !is_audio)
                    || matches!(self.deferred_callback_phase, Some(DeferredCallbackHostPhase::Visual) if is_audio)
                {
                    return Ok(S::Value(mms::Value::Null));
                }
                self.legacy_method_fallbacks += 1;
                let id = self.existing_id(component, "invoke_component_method")?;
                let args = args
                    .into_iter()
                    .map(external_value_to_legacy)
                    .collect::<Result<Vec<_>, _>>()?;
                let value = crate::scripting::component_method_registry::invoke_component_method(
                    self.world,
                    id,
                    &component_type,
                    &method,
                    &args,
                    |intent| self.intents.push(intent),
                )
                .map_err(|e| mms::HostError::failure("invoke_component_method", e))?;
                Ok(S::Value(legacy_value_to_external(value)?))
            }
            R::RegisterSignalHandler {
                operation_id,
                scope,
                name,
                callback,
            } => {
                let Some(binding) = self
                    .bindings
                    .and_then(|bindings| bindings.get(operation_id))
                else {
                    return Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: "operation ID is not present in the Mittens binding table".into(),
                    });
                };
                let super::runtime_config::MittensBinding::Signal { name: signal } = binding else {
                    return Err(mms::HostError {
                        kind: mms::HostErrorKind::InvalidRequest,
                        operation: format!("{operation_id:?}"),
                        message: format!("{binding:?} cannot register a signal handler"),
                    });
                };
                self.record_signal_route(signal, scope, name, callback)
            }
            R::RegisterSignalHandlerByName {
                scope,
                signal,
                name,
                callback,
            } => self.record_signal_route(&signal, scope, name, callback),
            R::LoadSource {
                importer,
                specifier,
            } => {
                let requested = std::path::Path::new(&specifier);
                let path = if requested.is_absolute() {
                    requested.to_path_buf()
                } else {
                    let importer = importer.as_ref().ok_or_else(|| mms::HostError {
                        kind: mms::HostErrorKind::SourceFailure,
                        operation: "load_source".into(),
                        message: format!("relative import '{specifier}' has no importer identity"),
                    })?;
                    std::path::Path::new(importer.as_str())
                        .parent()
                        .ok_or_else(|| mms::HostError {
                            kind: mms::HostErrorKind::SourceFailure,
                            operation: "load_source".into(),
                            message: format!("importer '{}' has no parent", importer.as_str()),
                        })?
                        .join(requested)
                };
                let path = path.canonicalize().map_err(|error| mms::HostError {
                    kind: mms::HostErrorKind::SourceFailure,
                    operation: "load_source".into(),
                    message: format!("cannot resolve '{}': {error}", path.display()),
                })?;
                let source = std::fs::read_to_string(&path).map_err(|error| mms::HostError {
                    kind: mms::HostErrorKind::SourceFailure,
                    operation: "load_source".into(),
                    message: format!("cannot read '{}': {error}", path.display()),
                })?;
                Ok(S::Source(mms::LoadedSource {
                    identity: mms::SourceId::new(path.to_string_lossy()),
                    source,
                }))
            }
        }
    }
}

fn reject_animation_legacy_fallback(
    tree: &mms::MaterializedCE,
    operation: &str,
) -> Result<(), mms::HostError> {
    fn contains_animation(tree: &mms::MaterializedCE) -> bool {
        matches!(tree.component_type.as_str(), "Animation" | "Keyframe")
            || tree.children.iter().any(|child| match child {
                mms::CeChild::Spawn(child) => contains_animation(child),
                mms::CeChild::Attach(_) => false,
            })
    }
    if contains_animation(tree) {
        return Err(mms::HostError::failure(
            operation,
            "Animation/Keyframe trees must use the RuntimeSpec direct path",
        ));
    }
    Ok(())
}

fn file_read_text(args: Vec<mms::TransportValue>) -> Result<mms::HostResponse, mms::HostError> {
    let [mms::TransportValue::String(path)] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "File.read_text",
            "expected one string path argument",
        ));
    };
    let text = std::fs::read_to_string(path).map_err(|error| {
        mms::HostError::failure("File.read_text", format!("cannot read '{path}': {error}"))
    })?;
    Ok(mms::HostResponse::Transport(mms::TransportValue::String(
        text,
    )))
}

fn audio_input_devices(
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    use cpal::traits::{DeviceTrait, HostTrait};

    if !args.is_empty() {
        return Err(mms::HostError::failure(
            "AudioInput.devices",
            "expects no arguments",
        ));
    }
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|error| {
        mms::HostError::failure(
            "AudioInput.devices",
            format!("cannot enumerate inputs: {error}"),
        )
    })?;
    let names = devices
        .filter_map(|device| device.name().ok())
        .map(mms::TransportValue::String)
        .collect();
    Ok(mms::HostResponse::Transport(mms::TransportValue::Array(
        names,
    )))
}

fn audio_output_devices(
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    use cpal::traits::{DeviceTrait, HostTrait};

    if !args.is_empty() {
        return Err(mms::HostError::failure(
            "AudioOutput.devices",
            "expects no arguments",
        ));
    }
    let host = cpal::default_host();
    let devices = host.output_devices().map_err(|error| {
        mms::HostError::failure(
            "AudioOutput.devices",
            format!("cannot enumerate outputs: {error}"),
        )
    })?;
    let names = devices
        .filter_map(|device| device.name().ok())
        .map(mms::TransportValue::String)
        .collect();
    Ok(mms::HostResponse::Transport(mms::TransportValue::Array(
        names,
    )))
}

fn json_parse(args: Vec<mms::TransportValue>) -> Result<mms::HostResponse, mms::HostError> {
    let [mms::TransportValue::String(text)] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "JSON.parse",
            "expected one string argument",
        ));
    };
    let value = serde_json::from_str(text)
        .map_err(|error| mms::HostError::failure("JSON.parse", error.to_string()))?;
    Ok(mms::HostResponse::Transport(json_to_transport(value)?))
}

fn json_stringify(args: Vec<mms::TransportValue>) -> Result<mms::HostResponse, mms::HostError> {
    let [value] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "JSON.stringify",
            "expected one argument",
        ));
    };
    let text = serde_json::to_string(&transport_to_json(value)?)
        .map_err(|error| mms::HostError::failure("JSON.stringify", error.to_string()))?;
    Ok(mms::HostResponse::Transport(mms::TransportValue::String(
        text,
    )))
}

fn json_to_transport(value: serde_json::Value) -> Result<mms::TransportValue, mms::HostError> {
    Ok(match value {
        serde_json::Value::Null => mms::TransportValue::Null,
        serde_json::Value::Bool(value) => mms::TransportValue::Bool(value),
        serde_json::Value::Number(value) => {
            mms::TransportValue::Number(value.as_f64().ok_or_else(|| {
                mms::HostError::failure("JSON.parse", "number cannot be represented by MMS")
            })?)
        }
        serde_json::Value::String(value) => mms::TransportValue::String(value),
        serde_json::Value::Array(values) => mms::TransportValue::Array(
            values
                .into_iter()
                .map(json_to_transport)
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(values) => mms::TransportValue::Table(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, json_to_transport(value)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
    })
}

fn transport_to_json(value: &mms::TransportValue) -> Result<serde_json::Value, mms::HostError> {
    Ok(match value {
        mms::TransportValue::Null => serde_json::Value::Null,
        mms::TransportValue::Bool(value) => serde_json::Value::Bool(*value),
        mms::TransportValue::Number(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| mms::HostError::failure("JSON.stringify", "numbers must be finite"))?,
        mms::TransportValue::String(value) => serde_json::Value::String(value.clone()),
        mms::TransportValue::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(transport_to_json)
                .collect::<Result<_, _>>()?,
        ),
        mms::TransportValue::Table(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), transport_to_json(value)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
        mms::TransportValue::Component(_) => {
            return Err(mms::HostError::failure(
                "JSON.stringify",
                "component handles are not JSON values",
            ));
        }
        mms::TransportValue::Callback(_) => {
            return Err(mms::HostError::failure(
                "JSON.stringify",
                "functions are not JSON values",
            ));
        }
    })
}

fn signal_kind(name: &str) -> Option<SignalKind> {
    Some(match name {
        "FrameTick" => SignalKind::FrameTick,
        "KeyDown" => SignalKind::KeyDown,
        "KeyUp" => SignalKind::KeyUp,
        "KeyPress" => SignalKind::KeyPress,
        "GLTFInitialized" => SignalKind::GltfInitialized,
        "Click" => SignalKind::Click,
        "ToggleChanged" => SignalKind::ToggleChanged,
        "SliderChanged" => SignalKind::SliderChanged,
        "SliderCommitted" => SignalKind::SliderCommitted,
        "DataEvent" => SignalKind::DataEvent,
        "CollisionStarted" => SignalKind::CollisionStarted,
        "CollisionEnded" => SignalKind::CollisionEnded,
        "DragStart" => SignalKind::DragStart,
        "GrabStart" => SignalKind::GrabStart,
        "GrabEnd" => SignalKind::GrabEnd,
        "MountStarted" => SignalKind::MountStarted,
        "MountEnded" => SignalKind::MountEnded,
        "DragMove" => SignalKind::DragMove,
        "DragEnd" => SignalKind::DragEnd,
        "ParentChanged" => SignalKind::ParentChanged,
        "RayIntersected" => SignalKind::RayIntersected,
        "Scrolling" => SignalKind::Scrolling,
        "TextInputChanged" => SignalKind::TextInputChanged,
        "TextInputFocusChanged" => SignalKind::TextInputFocusChanged,
        "SelectionAdded" => SignalKind::SelectionAdded,
        "SelectionRemoved" => SignalKind::SelectionRemoved,
        "SelectionChanged" => SignalKind::SelectionChanged,
        "SelectionCleared" => SignalKind::SelectionCleared,
        "XrButtonDown" => SignalKind::XrButtonDown,
        "XrButtonUp" => SignalKind::XrButtonUp,
        "XrButtonChanged" => SignalKind::XrButtonChanged,
        "XrAxisChanged" => SignalKind::XrAxisChanged,
        "HttpRequest" => SignalKind::HttpRequest,
        "HttpResponse" => SignalKind::HttpResponse,
        "HttpError" => SignalKind::HttpError,
        "XrEyeTrackingUpdated" => SignalKind::XrEyeTrackingUpdated,
        "XrEyeTrackingHtcUpdated" => SignalKind::XrEyeTrackingHtcUpdated,
        _ => return None,
    })
}

fn event_arg_transport(
    signal: &crate::engine::ecs::Signal,
) -> Result<mms::TransportValue, mms::HostError> {
    legacy_event_value_to_transport(crate::scripting::runner::event_arg_value(signal))
}

fn legacy_event_value_to_transport(
    value: legacy::Value,
) -> Result<mms::TransportValue, mms::HostError> {
    Ok(match value {
        legacy::Value::Null => mms::TransportValue::Null,
        legacy::Value::Bool(value) => mms::TransportValue::Bool(value),
        legacy::Value::Number(value) => mms::TransportValue::Number(value),
        legacy::Value::String(value) | legacy::Value::Identifier(value) => {
            mms::TransportValue::String(value)
        }
        legacy::Value::Array(values) => mms::TransportValue::Array(
            values
                .into_iter()
                .map(legacy_event_value_to_transport)
                .collect::<Result<_, _>>()?,
        ),
        legacy::Value::Map(values) => mms::TransportValue::Table(
            values
                .into_iter()
                .map(|(name, value)| Ok((name, legacy_event_value_to_transport(value)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
        legacy::Value::ComponentObject { id, .. } => {
            mms::TransportValue::Component(MittensHost::component_handle(id))
        }
        other => {
            return Err(mms::HostError {
                kind: mms::HostErrorKind::Conversion,
                operation: "signal_payload".into(),
                message: format!("event value {other:?} cannot enter the callback queue"),
            });
        }
    })
}

fn external_tree_to_legacy(
    tree: mms::MaterializedCE,
) -> Result<legacy::MaterializedCE, mms::HostError> {
    if matches!(tree.component_type.as_str(), "Animation" | "Keyframe") {
        return Err(mms::HostError::failure(
            "legacy component conversion",
            "Animation/Keyframe trees must use the RuntimeSpec direct path",
        ));
    }
    let ctor_method = tree.constructor.name;
    let ctor_args = tree.constructor.arguments;
    Ok(legacy::MaterializedCE {
        component_type: tree.component_type,
        component_property_assignment_only: tree.component_property_assignment_only,
        ctor_method,
        ctor_args: ctor_args
            .into_iter()
            .map(external_value_to_legacy)
            .collect::<Result<_, _>>()?,
        calls: tree
            .initializer_calls
            .into_iter()
            .map(|call| {
                Ok((
                    call.name,
                    call.arguments
                        .into_iter()
                        .map(external_value_to_legacy)
                        .collect::<Result<_, _>>()?,
                ))
            })
            .collect::<Result<_, mms::HostError>>()?,
        named: tree
            .properties
            .into_iter()
            .map(|property| Ok((property.name, external_value_to_legacy(property.value)?)))
            .collect::<Result<_, mms::HostError>>()?,
        positionals: tree
            .positionals
            .into_iter()
            .map(external_value_to_legacy)
            .collect::<Result<_, _>>()?,
        deferred_block: tree
            .deferred_block
            .map(|closure| {
                Ok(legacy::RuntimeClosure {
                    body: closure.body,
                    captured_env: Arc::new(
                        closure
                            .captured_env
                            .iter()
                            .map(|(k, v)| Ok((k.clone(), external_value_to_legacy(v.clone())?)))
                            .collect::<Result<HashMap<_, _>, mms::HostError>>()?,
                    ),
                    heap: legacy::HeapHandle::new(),
                    analysis: closure.analysis,
                })
            })
            .transpose()?,
        children: tree
            .children
            .into_iter()
            .map(|child| match child {
                mms::CeChild::Spawn(tree) => {
                    Ok(legacy::CeChild::Spawn(external_tree_to_legacy(tree)?))
                }
                mms::CeChild::Attach(handle) => {
                    Ok(legacy::CeChild::Attach(MittensHost::component_id(handle)))
                }
            })
            .collect::<Result<_, mms::HostError>>()?,
    })
}

fn external_value_to_legacy(value: mms::Value) -> Result<legacy::Value, mms::HostError> {
    Ok(match value {
        mms::Value::Null => legacy::Value::Null,
        mms::Value::Bool(v) => legacy::Value::Bool(v),
        mms::Value::Number(v) => legacy::Value::Number(v),
        mms::Value::String(v) => legacy::Value::String(v),
        mms::Value::Dimension { value, unit } => legacy::Value::Dimension { value, unit },
        mms::Value::Array(v) => legacy::Value::Array(
            v.into_iter()
                .map(external_value_to_legacy)
                .collect::<Result<_, _>>()?,
        ),
        mms::Value::Map(v) => legacy::Value::Map(
            v.into_iter()
                .map(|(k, v)| Ok((k, external_value_to_legacy(v)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
        mms::Value::ComponentObject { id, component_type } => legacy::Value::ComponentObject {
            id: MittensHost::component_id(id),
            component_type,
        },
        mms::Value::Identifier(v) => legacy::Value::Identifier(v),
        mms::Value::BuiltinTable(kind) => legacy::Value::BuiltinTable(match kind {
            mms::BuiltinTableKind::Math => legacy::BuiltinTableKind::Math,
            mms::BuiltinTableKind::MusicNote => legacy::BuiltinTableKind::MusicNote,
        }),
        mms::Value::ComponentExpr(tree) => {
            legacy::Value::ComponentExpr(Box::new(external_tree_to_legacy(*tree)?))
        }
        mms::Value::Function {
            params,
            body,
            captured_env,
            ..
        } => legacy::Value::Function {
            params,
            body,
            captured_env: Arc::new(
                captured_env
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), external_value_to_legacy(v.clone())?)))
                    .collect::<Result<_, mms::HostError>>()?,
            ),
            heap: legacy::HeapHandle::new(),
        },
        mms::Value::Object(id) => legacy::Value::Map(
            id.with_map(|map| map.clone())
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| Ok((k, external_value_to_legacy(v)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
        mms::Value::Module {
            named, sequence, ..
        } => legacy::Value::Module {
            named: named
                .into_iter()
                .map(|(k, v)| Ok((k, external_value_to_legacy(v)?)))
                .collect::<Result<_, mms::HostError>>()?,
            sequence: sequence
                .into_iter()
                .map(external_tree_to_legacy)
                .collect::<Result<_, _>>()?,
            heap: legacy::HeapHandle::new(),
        },
    })
}

fn legacy_value_to_external(value: legacy::Value) -> Result<mms::Value, mms::HostError> {
    Ok(match value {
        legacy::Value::Null => mms::Value::Null,
        legacy::Value::Bool(v) => mms::Value::Bool(v),
        legacy::Value::Number(v) => mms::Value::Number(v),
        legacy::Value::String(v) => mms::Value::String(v),
        legacy::Value::Dimension { value, unit } => mms::Value::Dimension { value, unit },
        legacy::Value::Array(v) => mms::Value::Array(
            v.into_iter()
                .map(legacy_value_to_external)
                .collect::<Result<_, _>>()?,
        ),
        legacy::Value::Map(v) => mms::Value::Map(
            v.into_iter()
                .map(|(k, v)| Ok((k, legacy_value_to_external(v)?)))
                .collect::<Result<_, mms::HostError>>()?,
        ),
        legacy::Value::ComponentObject { id, component_type } => mms::Value::ComponentObject {
            id: MittensHost::component_handle(id),
            component_type,
        },
        legacy::Value::Identifier(v) => mms::Value::Identifier(v),
        legacy::Value::BuiltinTable(kind) => mms::Value::BuiltinTable(match kind {
            legacy::BuiltinTableKind::Math => mms::BuiltinTableKind::Math,
            legacy::BuiltinTableKind::MusicNote => mms::BuiltinTableKind::MusicNote,
        }),
        other => {
            return Err(mms::HostError::failure(
                "value_conversion",
                format!("unsupported engine runtime value: {other:?}"),
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_handles_round_trip_losslessly() {
        let mut world = World::default();
        let id = world.add_component(crate::engine::ecs::component::DataComponent::default());
        assert_eq!(
            MittensHost::component_id(MittensHost::component_handle(id)),
            id
        );
    }

    #[test]
    fn runtime_spec_smoke_uses_no_legacy_component_conversion() {
        let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut command_queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        let invocations = Arc::new(Mutex::new(Vec::new()));
        let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
            .with_bindings(configured.bindings())
            .with_rx(&mut rx)
            .with_callback_invocations(invocations);
        let mut session = configured.runtime().session(host);

        session
            .eval(include_str!("../../examples/runtime-spec-smoke.mms"))
            .unwrap();
        session.eval("let cube = R.cube() {} T { cube }").unwrap();

        assert_eq!(session.host().legacy_component_fallbacks(), 0);
    }

    #[test]
    fn configured_components_outside_the_direct_slice_use_the_explicit_fallback() {
        let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
        let mut world = World::default();
        let mut command_queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
            .with_bindings(configured.bindings());
        let mut session = configured.runtime().session(host);

        session.eval("Grid.spacing(1.0) {}").unwrap();

        assert_eq!(session.host().legacy_component_fallbacks(), 1);
    }

    #[test]
    fn configured_component_methods_dispatch_by_operation_id() {
        use mms::Host;

        let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
        let translation = configured
            .spec()
            .component("Transform")
            .unwrap()
            .method("translation")
            .unwrap()
            .operation_id()
            .unwrap();
        let smoke_api = configured
            .spec()
            .api(Some("mittens"), "smoke")
            .unwrap()
            .operation_id();

        let mut world = World::default();
        let id = world.add_component(
            crate::engine::ecs::component::TransformComponent::new().with_position(1.0, 2.0, 3.0),
        );
        let mut command_queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        let mut host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
            .with_bindings(configured.bindings());

        let response = host
            .dispatch(mms::HostRequest::InvokeComponentMethod {
                operation_id: translation,
                component: MittensHost::component_handle(id),
                args: Vec::new(),
            })
            .unwrap();
        assert_eq!(
            response,
            mms::HostResponse::Value(mms::Value::Array(vec![
                mms::Value::Number(1.0),
                mms::Value::Number(2.0),
                mms::Value::Number(3.0),
            ]))
        );

        let error = host
            .dispatch(mms::HostRequest::InvokeComponentMethod {
                operation_id: smoke_api,
                component: MittensHost::component_handle(id),
                args: Vec::new(),
            })
            .unwrap_err();
        assert_eq!(error.kind, mms::HostErrorKind::InvalidRequest);
        assert!(
            error
                .message
                .contains("cannot be invoked as a component method")
        );
    }

    #[test]
    fn configured_method_call_never_uses_the_by_name_compatibility_request() {
        let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
        let mut world = World::default();
        let id = world.add_component(
            crate::engine::ecs::component::TransformComponent::new().with_position(4.0, 5.0, 6.0),
        );
        world.get_component_record_mut(id).unwrap().name = "target".into();
        let mut command_queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
            .with_bindings(configured.bindings());
        let mut session = configured.runtime().session(host);

        session.eval("query(\"#target\").translation()").unwrap();

        assert_eq!(session.host().legacy_method_fallbacks(), 0);
    }

    #[test]
    fn configured_signal_registration_resolves_to_an_opaque_mittens_route() {
        let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
        let click_id = configured.spec().signal("Click").unwrap().operation_id();
        assert_eq!(
            configured.bindings().get(click_id),
            Some(&super::super::runtime_config::MittensBinding::Signal { name: "Click" })
        );

        let mut world = World::default();
        let root = world.add_component(crate::engine::ecs::component::TransformComponent::new());
        world.get_component_record_mut(root).unwrap().name = "signal-root".into();
        let mut command_queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        let mut routes = Vec::new();
        let mut rx = RxWorld::default();
        let invocations = Arc::new(Mutex::new(Vec::new()));
        let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
            .with_bindings(configured.bindings())
            .with_signal_routes(&mut routes)
            .with_rx(&mut rx)
            .with_callback_invocations(Arc::clone(&invocations));
        let mut session = configured.runtime().session(host);

        session
            .eval("on(query(\"#signal-root\"), \"Click\", fn(event) {})")
            .unwrap();
        drop(session);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].kind, SignalKind::Click);
        assert!(routes[0].scope.is_some());

        rx.dispatch_event_handlers(
            &mut world,
            &crate::engine::ecs::Signal::event(
                root,
                crate::engine::ecs::EventSignal::Click {
                    raycaster: ComponentId::default(),
                    renderable: root,
                    hit_point: [0.0; 3],
                    screen_pos_px: None,
                },
            ),
        );
        let invocations = invocations.lock().unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].callback, routes[0].callback);
        assert_eq!(invocations[0].args, vec![mms::TransportValue::Null]);
    }

    #[test]
    fn let_bound_component_attaches_and_its_signal_callback_uses_the_live_handle() {
        let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
        let mut world = World::default();
        let mut command_queue = crate::engine::ecs::CommandQueue::new();
        let mut intents = Vec::new();
        let mut routes = Vec::new();
        let mut rx = RxWorld::default();
        let invocations = Arc::new(Mutex::new(Vec::new()));
        let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
            .with_bindings(configured.bindings())
            .with_signal_routes(&mut routes)
            .with_rx(&mut rx)
            .with_callback_invocations(Arc::clone(&invocations));
        let mut session = configured.runtime().session(host);

        session
            .eval(
                r#"
                let button = Transform.position(1.0, 2.0, 3.0) { name = "live-button" }
                on(button, "Click", fn(event) { return button.translation() })
                Transform { name = "root" button }
                "#,
            )
            .unwrap();

        assert_eq!(session.host().legacy_component_fallbacks(), 0);
        assert_eq!(session.host().legacy_method_fallbacks(), 0);
        let button = session.host().signal_routes().unwrap()[0].scope.unwrap();
        let event = crate::engine::ecs::Signal::event(
            button,
            crate::engine::ecs::EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: button,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        );
        session.host_mut().dispatch_event_handlers(&event).unwrap();
        let invocation = invocations.lock().unwrap().remove(0);
        let result = session.invoke_callback_invocation(invocation).unwrap();

        assert_eq!(
            result,
            mms::Value::Array(vec![
                mms::Value::Number(1.0),
                mms::Value::Number(2.0),
                mms::Value::Number(3.0),
            ])
        );
        assert_eq!(session.host().legacy_method_fallbacks(), 0);
    }
}
