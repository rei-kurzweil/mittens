#[path = "benchmark_api.rs"]
pub(super) mod benchmark_api;
#[path = "graphics_settings_api.rs"]
pub(super) mod graphics_settings_api;
#[path = "xr_settings_api.rs"]
pub(super) mod xr_settings_api;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
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

#[path = "host_signals.rs"]
mod signals;

#[path = "host_context.rs"]
mod context;
pub(crate) use context::MittensHost;

impl<'a, 'bindings> MittensHost<'a, 'bindings> {
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

    fn emit_data(
        &mut self,
        args: Vec<mms::TransportValue>,
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::TransportValue as V;

        if self.deferred_callback_phase == Some(DeferredCallbackHostPhase::Audio) {
            return Ok(mms::HostResponse::Unit);
        }

        let (scope, name, payload) = match args.as_slice() {
            [scope, V::String(name)] => (scope, name, None),
            [scope, V::String(name), payload] => (scope, name, Some(payload)),
            _ => {
                return Err(mms::HostError::failure(
                    "emit_data",
                    "expects a component scope, a name, and an optional component payload",
                ));
            }
        };
        let component = |value: &V| match value {
            V::Component(handle) | V::TypedComponent { handle, .. } => {
                self.existing_id(*handle, "emit_data")
            }
            _ => Err(mms::HostError::failure(
                "emit_data",
                "scope and non-null payload must be component handles",
            )),
        };
        let scope = component(scope)?;
        let payload = match payload {
            None | Some(V::Null) => None,
            Some(value) => Some(component(value)?),
        };
        // Queue the event for the normal Rx/callback boundary. Host dispatch
        // must not re-enter the MMS session to deliver handlers synchronously.
        self.emit.push_event(
            scope,
            crate::engine::ecs::EventSignal::DataEvent {
                name: name.clone(),
                payload,
            },
        );
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
            | mms::HostRequest::RegisterSignalHandlerByName { callback, .. }
            | mms::HostRequest::SetEventBindingBypass { callback, .. }
            | mms::HostRequest::RemoveEventBinding { callback } => Some(*callback),
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
        #[cfg(test)]
        let _cpu_scope = crate::engine::update_cpu_profile::host_request(&request);
        use mms::{HostRequest as R, HostResponse as S};
        match request {
            R::ValidateComponents { components } => Ok(S::Components(
                components
                    .into_iter()
                    .filter_map(|handle| {
                        self.world
                            .component_name(Self::component_id(handle))
                            .map(|ty| (handle, ty.to_owned()))
                    })
                    .collect(),
            )),
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
                    super::runtime_config::MittensBinding::Api(api)
                        if xr_settings_api::supports(*api) =>
                    {
                        self.invoke_xr_settings(*api, args)
                    }
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
                    super::runtime_config::MittensBinding::Api(api)
                        if benchmark_api::supports(*api) =>
                    {
                        self.invoke_benchmark(*api, args)
                    }
                    super::runtime_config::MittensBinding::Api(api)
                        if graphics_settings_api::supports(*api) =>
                    {
                        self.invoke_graphics_settings(*api, args)
                    }
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::EmitData,
                    ) => self.emit_data(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::FileReadText,
                    ) => super::file_api::read_text(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::FileListDir,
                    ) => super::file_api::list_dir(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::JsonTryParseObject,
                    ) => super::json_api::try_parse_object(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::JsonParse,
                    ) => super::json_api::parse(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::JsonStringify,
                    ) => super::json_api::stringify(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::AudioInputDevices,
                    ) => audio_input_devices(args),
                    super::runtime_config::MittensBinding::Api(
                        super::runtime_config::MittensApi::AudioOutputDevices,
                    ) => audio_output_devices(args),
                    super::runtime_config::MittensBinding::Api(api)
                        if super::file_browser::supports(*api) =>
                    {
                        super::file_browser::invoke(self, *api, args)
                    }
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
                let mut tree = tree;
                super::rigging_binding::materialize_descendants(self, &mut tree)?;
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
                let mut tree = tree;
                if let Some(bindings) = self.bindings {
                    super::configured_registry::materialize_material_children(
                        &mut tree, bindings, self.world, self.emit,
                    )
                    .map_err(|error| mms::HostError::failure("spawn", error))?;
                }
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
                let mut tree = tree;
                super::rigging_binding::materialize_descendants(self, &mut tree)?;
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
                let mut tree = tree;
                if let Some(bindings) = self.bindings {
                    super::configured_registry::materialize_material_children(
                        &mut tree, bindings, self.world, self.emit,
                    )
                    .map_err(|error| mms::HostError::failure("register", error))?;
                }
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
                use crate::engine::ecs::component::{QueryRootMode, parse_scoped_query};
                let scoped = parse_scoped_query(&selector);
                let scope_id = scope
                    .map(|handle| self.existing_id(handle, "query"))
                    .transpose()?;
                let roots = match scoped.root_mode {
                    QueryRootMode::WorldRoot => self.world.world_roots(),
                    QueryRootMode::ParentScope { levels_up } => {
                        let mut parent = scope_id;
                        for _ in 0..levels_up {
                            parent = parent.and_then(|id| self.world.parent_of(id));
                        }
                        parent.into_iter().collect()
                    }
                    QueryRootMode::SelfSubtree => scope_id
                        .map(|id| self.world.scripting_query_roots(id))
                        .unwrap_or_else(|| self.world.world_roots()),
                };
                let selector = scoped.selector;
                let mut matches = Vec::new();
                for root in roots {
                    if multiple {
                        matches.extend(self.world.find_all_components(root, selector));
                    } else if let Some(id) = self.world.find_component(root, selector) {
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
                if *component_type == "Transform"
                    && matches!(*method, "set_translation" | "set_rotation_quat")
                {
                    let count = if *method == "set_translation" { 3 } else { 4 };
                    let Some(mms::Value::Array(values)) = args.first() else {
                        return Err(mms::HostError::failure(*method, "expected numeric vector"));
                    };
                    if values.len() != count {
                        return Err(mms::HostError::failure(*method, "incorrect vector length"));
                    }
                    let mut numeric = [0.; 4];
                    for (out, value) in numeric.iter_mut().zip(values) {
                        let mms::Value::Number(value) = value else {
                            return Err(mms::HostError::failure(
                                *method,
                                "expected finite numbers",
                            ));
                        };
                        *out = *value as f32;
                        if !out.is_finite() {
                            return Err(mms::HostError::failure(
                                *method,
                                "expected finite f32 values",
                            ));
                        }
                    }
                    if count == 4 && { let norm = numeric.iter().map(|v| v * v).sum::<f32>(); !norm.is_finite() || norm < 1e-12 } {
                        return Err(mms::HostError::failure(*method, "degenerate quaternion"));
                    }
                    let space = match args.get(1) {
                        None => crate::engine::transform::TransformSpace::Local,
                        Some(mms::Value::String(s)) if s == "local" => {
                            crate::engine::transform::TransformSpace::Local
                        }
                        Some(mms::Value::String(s)) if s == "world" => {
                            crate::engine::transform::TransformSpace::World
                        }
                        _ => {
                            return Err(mms::HostError::failure(
                                *method,
                                "space must be local or world",
                            ));
                        }
                    };
                    self.intents.push(IntentValue::PatchTransformTrs {
                        component_id: id,
                        space,
                        translation: (count == 3).then(|| numeric[..3].try_into().unwrap()),
                        rotation: (count == 4).then_some(numeric),
                    });
                    return Ok(S::Value(mms::Value::Null));
                }
                if *component_type == "GLTF" && matches!(*method, "morph_weight" | "set_morph_weight") {
                    return super::morph_api::invoke(self.world, id, method, &args)
                        .map(S::Value)
                        .map_err(|e| mms::HostError::failure(format!("GLTF.{method}"), e));
                }
                if super::udp_channel_api::supports(component_type) {
                    return self.invoke_udp(id, component_type, method, args);
                }
                if (*component_type == "Transform" && matches!(*method, "trs" | "world_trs"))
                    || (*component_type == "TransformWorld" && *method == "trs")
                {
                    use crate::engine::ecs::component::TransformComponent;
                    use crate::engine::transform::{TransformSpace, TransformTrs};
                    let space = if *method == "world_trs" || *component_type == "TransformWorld" {
                        TransformSpace::World
                    } else {
                        TransformSpace::Local
                    };
                    let error = |message| mms::HostError::failure("Transform.trs", message);
                    match args.as_slice() {
                        [] => {
                            let pose = if space == TransformSpace::World {
                                crate::engine::ecs::system::TransformSystem::world_trs(
                                    self.world, id,
                                )
                                .map_err(|e| error(e.to_string()))?
                            } else {
                                self.world
                                    .get_component_by_id_as::<TransformComponent>(id)
                                    .ok_or_else(|| error("receiver is not a Transform".into()))?
                                    .trs()
                            };
                            let vector = |values: &[f32]| {
                                mms::Value::Array(
                                    values
                                        .iter()
                                        .map(|v| mms::Value::Number(*v as f64))
                                        .collect(),
                                )
                            };
                            return Ok(S::Value(mms::Value::Map(HashMap::from([
                                ("translation".into(), vector(&pose.translation)),
                                (
                                    "rotation_quat_xyzw".into(),
                                    vector(&pose.rotation_quat_xyzw),
                                ),
                                ("scale".into(), vector(&pose.scale)),
                            ]))));
                        }
                        [value] => {
                            let fields = match value {
                                mms::Value::Map(fields) => fields.clone(),
                                mms::Value::Object(id) => id
                                    .with_map(Clone::clone)
                                    .ok_or_else(|| error("stale TRS table".into()))?,
                                _ => return Err(error("expected a copied TRS table".into())),
                            };
                            fn vector<const N: usize>(
                                fields: &HashMap<String, mms::Value>,
                                name: &str,
                            ) -> Result<[f32; N], mms::HostError> {
                                let Some(mms::Value::Array(values)) = fields.get(name) else {
                                    return Err(mms::HostError::failure(
                                        "Transform.trs",
                                        format!("missing {name} array"),
                                    ));
                                };
                                if values.len() != N {
                                    return Err(mms::HostError::failure(
                                        "Transform.trs",
                                        format!("{name} requires {N} values"),
                                    ));
                                }
                                let mut result = [0.; N];
                                for (out, value) in result.iter_mut().zip(values) {
                                    let mms::Value::Number(value) = value else {
                                        return Err(mms::HostError::failure(
                                            "Transform.trs",
                                            "TRS values must be numbers",
                                        ));
                                    };
                                    *out = *value as f32;
                                }
                                Ok(result)
                            }
                            let pose = TransformTrs::new(
                                vector(&fields, "translation")?,
                                vector(&fields, "rotation_quat_xyzw")?,
                                vector(&fields, "scale")?,
                            )
                            .normalized()
                            .map_err(|e| error(e.to_string()))?;
                            self.intents.push(IntentValue::SetTransformTrs {
                                component_id: id,
                                space,
                                value: pose,
                            });
                            return Ok(S::Value(mms::Value::Null));
                        }
                        _ => return Err(error("expects zero arguments or one TRS table".into())),
                    }
                }
                if *component_type == "Raycast" && *method == "request_raycast" {
                    self.world
                        .get_component_by_id_as::<crate::engine::ecs::component::RayCastComponent>(
                            id,
                        )
                        .ok_or_else(|| {
                            mms::HostError::failure(
                                "Raycast.request_raycast",
                                "receiver is not a Raycast",
                            )
                        })?;
                    self.intents
                        .push(IntentValue::RequestRaycast { component_id: id });
                    return Ok(S::Value(mms::Value::Null));
                }
                if *component_type == "AudioClip" && *method == "play" {
                    self.world
                        .get_component_by_id_as::<crate::engine::ecs::component::AudioClipComponent>(id)
                        .ok_or_else(|| mms::HostError::failure("AudioClip.play", "receiver is not an AudioClip"))?;
                    self.intents.push(IntentValue::AudioSchedulePlay {
                        component_id: id,
                        beat_offset: 0.0,
                        beat_context: None,
                        note: None,
                        gain: None,
                        rate: None,
                        duration: None,
                    });
                    return Ok(S::Value(mms::Value::Null));
                }
                if *component_type == "AvatarControl" && *method == "eyelid_tracking" {
                    let avatar=self.world.get_component_by_id_as::<crate::engine::ecs::component::AvatarControlComponent>(id)
                        .ok_or_else(||mms::HostError::failure("eyelid_tracking","not an AvatarControl"))?;
                    let [left, right] = avatar.eye_blink_state.tracked();
                    return Ok(S::Value(mms::Value::Map(HashMap::from([
                        ("left".into(), mms::Value::Bool(left)),
                        ("right".into(), mms::Value::Bool(right)),
                    ]))));
                }
                if *component_type == "XRHand" && matches!(*method, "pose_valid" | "set_pose_valid")
                {
                    let hand = self.world.get_component_by_id_as_mut::<crate::engine::ecs::component::ControllerXRComponent>(id)
                        .ok_or_else(|| mms::HostError::failure(*method, "receiver is not XRHand"))?;
                    if *method == "pose_valid" {
                        return Ok(S::Value(mms::Value::Bool(hand.pose_valid)));
                    }
                    if !hand.externally_driven {
                        return Err(mms::HostError::failure(
                            *method,
                            "local tracking owns this hand",
                        ));
                    }
                    let [mms::Value::Bool(valid)] = args.as_slice() else {
                        return Err(mms::HostError::failure(*method, "expected boolean"));
                    };
                    hand.pose_valid = *valid;
                    return Ok(S::Value(mms::Value::Null));
                }
                if *component_type == "XRHand" && *method == "get_grip_pose" {
                    let value = super::controller_pose_api::get_grip_pose(self.world, id)
                        .map_err(|e| mms::HostError::failure("XRHand.get_grip_pose", e))?;
                    return Ok(S::Value(value));
                }
                if *component_type == "Material" {
                    let value =
                        super::configured_registry::material_method(self.world, id, method, &args)
                            .map_err(|e| mms::HostError::failure("Material", e))?;
                    return Ok(S::Value(value));
                }
                if *component_type == "Armature"
                    && matches!(*method, "give_automatic_weights" | "status")
                {
                    let value = super::rigging_binding::invoke(self.world, id, method)
                        .map_err(|e| mms::HostError::failure("Armature", e))?;
                    return Ok(S::Value(value));
                }
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
            R::SetEventBindingBypass { callback, bypassed } => {
                self.change_event_binding(callback, Some(bypassed))
            }
            R::RemoveEventBinding { callback } => self.change_event_binding(callback, None),
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
        "DragStart" => SignalKind::DragStart,
        "GrabStart" => SignalKind::GrabStart,
        "GrabEnd" => SignalKind::GrabEnd,
        "PointerGrabbed" => SignalKind::PointerGrabbed,
        "PointerReleased" => SignalKind::PointerReleased,
        "ProjectileHit" => SignalKind::ProjectileHit,
        "ZoneEntered" => SignalKind::ZoneEntered,
        "ZoneExited" => SignalKind::ZoneExited,
        "MountStarted" => SignalKind::MountStarted,
        "MountEnded" => SignalKind::MountEnded,
        "DragMove" => SignalKind::DragMove,
        "DragEnd" => SignalKind::DragEnd,
        "ParentChanged" => SignalKind::ParentChanged,
        "AttachmentChanged" => SignalKind::AttachmentChanged,
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
        "UdpSample" => SignalKind::UdpSample,
        "UdpState" => SignalKind::UdpState,
        "MultiplayerConnected" => SignalKind::MultiplayerConnected,
        "MultiplayerPeerJoined" => SignalKind::MultiplayerPeerJoined,
        "MultiplayerPeerLeft" => SignalKind::MultiplayerPeerLeft,
        "MultiplayerMessage" => SignalKind::MultiplayerMessage,
        "MultiplayerError" => SignalKind::MultiplayerError,
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

pub(super) fn legacy_event_value_to_transport(
    value: legacy::Value,
) -> Result<mms::TransportValue, mms::HostError> {
    Ok(match value {
        legacy::Value::Null => mms::TransportValue::Null,
        legacy::Value::Bool(value) => mms::TransportValue::Bool(value),
        legacy::Value::Number(value) => mms::TransportValue::Number(value),
        legacy::Value::TypeTag(tag) => mms::TransportValue::TypeTag(tag),
        legacy::Value::RecordType(ty) => mms::TransportValue::RecordType(ty),
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
        legacy::Value::ComponentObject { id, component_type } => {
            mms::TransportValue::TypedComponent {
                handle: MittensHost::component_handle(id),
                component_type,
            }
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

pub(super) fn external_value_to_legacy(value: mms::Value) -> Result<legacy::Value, mms::HostError> {
    Ok(match value {
        mms::Value::EventBinding(_) => {
            return Err(mms::HostError::failure(
                "value_conversion",
                "event bindings cannot be passed through the legacy component API",
            ));
        }
        mms::Value::Null => legacy::Value::Null,
        mms::Value::Bool(v) => legacy::Value::Bool(v),
        mms::Value::Number(v) => legacy::Value::Number(v),
        mms::Value::TypeTag(tag) => legacy::Value::TypeTag(tag),
        mms::Value::RecordType(ty) => legacy::Value::RecordType(ty),
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
            body: body.as_ref().clone(),
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
        legacy::Value::TypeTag(tag) => mms::Value::TypeTag(tag),
        legacy::Value::RecordType(ty) => mms::Value::RecordType(ty),
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
#[path = "host_tests.rs"]
mod tests;
