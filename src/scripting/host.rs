use std::collections::HashMap;
use std::sync::Arc;

use meow_meow_script as mms;
use slotmap::{Key, KeyData};

use crate::engine::ecs::component::AudioClipComponent;
use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, SignalKind, World};
use crate::engine::graphics::RenderAssets;
use crate::scripting::object as legacy;

/// Engine implementation of the host-neutral Meow Meow host contract.
pub struct MittensHost<'a> {
    pub world: &'a mut World,
    pub rx: Option<&'a mut RxWorld>,
    pub render_assets: Option<&'a mut RenderAssets>,
    pub emit: &'a mut dyn SignalEmitter,
    pub intents: &'a mut Vec<IntentValue>,
}

impl<'a> MittensHost<'a> {
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
}

impl mms::Host for MittensHost<'_> {
    fn capabilities(&self) -> mms::HostCapabilities {
        crate::scripting::component_registry::SUPPORTED_COMPONENT_NAMES
            .iter()
            .fold(mms::HostCapabilities::default(), |capabilities, name| {
                capabilities.supports_component(*name)
            })
    }

    fn dispatch(&mut self, request: mms::HostRequest) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostRequest as R, HostResponse as S};
        match request {
            R::Emit { tree } => {
                let component_type = tree.component_type.clone();
                let response = self.dispatch(R::Spawn { tree })?;
                let S::Component { handle: native, .. } = response else {
                    return Err(mms::HostError::failure("emit", "spawn did not return a component"));
                };
                Ok(S::Component { handle: native, component_type })
            }
            R::RegisterComponent { tree } => {
                let component_type = tree.component_type.clone();
                let response = self.dispatch(R::Register { tree })?;
                let S::Component { handle: native, .. } = response else {
                    return Err(mms::HostError::failure("register_component", "registration did not return a component"));
                };
                Ok(S::Component { handle: native, component_type })
            }
            R::CallApi { api_id, .. } => Err(mms::HostError::unsupported(api_id)),
            R::Spawn { tree } => {
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
                    vec![self.existing_id(scope, "query")?]
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
                component,
                component_type,
                method,
                args,
            } => {
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
            R::AudioClipInstance {
                source,
                start_beat,
                stop_beat,
            } => {
                let source = self.existing_id(source, "audio_clip_instance")?;
                let source = self
                    .world
                    .get_component_by_id_as::<AudioClipComponent>(source)
                    .ok_or_else(|| {
                        mms::HostError::failure("audio_clip_instance", "source is not an AudioClip")
                    })?;
                let mut clip = AudioClipComponent::instance_of(source);
                if let Some(start) = start_beat {
                    clip.start_beat = start;
                }
                clip.stop_beat = stop_beat;
                let id = self.world.add_component(clip);
                Ok(S::Component {
                    handle: Self::component_handle(id),
                    component_type: "AudioClip".into(),
                })
            }
            R::RegisterHandler {
                scope,
                signal,
                name,
                handler,
            } => {
                let scope = self.existing_id(scope, "register_handler")?;
                let kind = signal_kind(&signal).ok_or_else(|| {
                    mms::HostError::failure(
                        "register_handler",
                        format!("unknown signal '{signal}'"),
                    )
                })?;
                let handler = external_value_to_legacy(handler)?;
                let Some(rx) = self.rx.as_deref_mut() else {
                    return Err(mms::HostError::unsupported("register_handler"));
                };
                let callback =
                    move |world: &mut World,
                          emit: &mut dyn SignalEmitter,
                          signal: &crate::engine::ecs::Signal| {
                        let arg = crate::scripting::runner::event_arg_value(signal);
                        if let Err(error) = crate::scripting::world_evaluator::eval_mms_fn(
                            &handler,
                            vec![arg],
                            None,
                            Some(world),
                            Some(emit),
                        ) {
                            eprintln!("[mms] handler error: {error}");
                        }
                    };
                if let Some(name) = name {
                    rx.add_handler_closure_named(kind, scope, Some(name), callback);
                } else {
                    rx.add_handler_closure(kind, scope, callback);
                }
                Ok(S::Unit)
            }
            R::AudioOperation {
                operation,
                target,
                args,
            } => {
                let targets = target.into_iter().collect();
                self.dispatch(R::EngineMutation {
                    operation,
                    targets,
                    args,
                })
            }
            R::EngineMutation {
                operation,
                targets,
                args,
            } => {
                // The legacy engine still owns its concrete mutation enum. Route
                // named operations through component-method dispatch where possible.
                let Some(target) = targets.first().copied() else {
                    return Err(mms::HostError::failure(
                        &operation,
                        "mutation requires a target",
                    ));
                };
                let id = self.existing_id(target, &operation)?;
                let component_type = self
                    .world
                    .component_name(id)
                    .unwrap_or("Component")
                    .to_owned();
                let args = args
                    .into_iter()
                    .map(external_value_to_legacy)
                    .collect::<Result<Vec<_>, _>>()?;
                let value = crate::scripting::component_method_registry::invoke_component_method(
                    self.world,
                    id,
                    &component_type,
                    &operation,
                    &args,
                    |intent| self.intents.push(intent),
                )
                .map_err(|e| mms::HostError::failure(&operation, e))?;
                Ok(S::Value(legacy_value_to_external(value)?))
            }
            R::ReplTree { .. } | R::ReplDump { .. } | R::ReplHelp | R::ReplClear => Ok(S::Unit),
        }
    }
}

fn signal_kind(name: &str) -> Option<SignalKind> {
    Some(match name {
        "FrameTick" => SignalKind::FrameTick,
        "Click" => SignalKind::Click,
        "DataEvent" => SignalKind::DataEvent,
        "CollisionStarted" => SignalKind::CollisionStarted,
        "CollisionEnded" => SignalKind::CollisionEnded,
        "DragStart" => SignalKind::DragStart,
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
        _ => return None,
    })
}

fn external_tree_to_legacy(
    tree: mms::MaterializedCE,
) -> Result<legacy::MaterializedCE, mms::HostError> {
    Ok(legacy::MaterializedCE {
        component_type: tree.component_type,
        component_property_assignment_only: tree.component_property_assignment_only,
        ctor_method: tree.ctor_method,
        ctor_args: tree
            .ctor_args
            .into_iter()
            .map(external_value_to_legacy)
            .collect::<Result<_, _>>()?,
        calls: tree
            .calls
            .into_iter()
            .map(|(name, args)| {
                Ok((
                    name,
                    args.into_iter()
                        .map(external_value_to_legacy)
                        .collect::<Result<_, _>>()?,
                ))
            })
            .collect::<Result<_, mms::HostError>>()?,
        named: tree
            .named
            .into_iter()
            .map(|(name, value)| Ok((name, external_value_to_legacy(value)?)))
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
}
