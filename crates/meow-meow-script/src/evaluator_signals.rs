//! World/component event observation and binding resource lifetime.
use super::*;

impl<H: Host> Evaluator<'_, H> {
    pub(super) fn register_signal_handler(
        &mut self,
        args: &[Value],
        global: bool,
    ) -> Result<Value, EvalError> {
        let (scope, signal_index) = if global {
            (None, 0)
        } else {
            let scope = match args.first() {
                Some(Value::ComponentObject { id, .. }) => Some(*id),
                Some(Value::Identifier(value)) if value == "world" || value == "__mms_world__" => {
                    None
                }
                _ => {
                    return Err(EvalError::Runtime(
                        "on expects a live component or world as argument 0".into(),
                    ));
                }
            };
            (scope, 1)
        };
        let signal = match args.get(signal_index) {
            Some(Value::String(signal)) => signal.clone(),
            _ => {
                return Err(EvalError::Runtime(format!(
                    "{} expects a signal name string as argument {signal_index}",
                    if global { "on_global" } else { "on" }
                )));
            }
        };
        let trailing = &args[signal_index + 1..];
        let (name, function) = match trailing {
            [function @ Value::Function { .. }] => (None, function.clone()),
            [Value::String(name), function @ Value::Function { .. }] => {
                (Some(name.clone()), function.clone())
            }
            _ => {
                return Err(EvalError::Runtime(format!(
                    "{} expects a callback, optionally preceded by a handler name",
                    if global { "on_global" } else { "on" }
                )));
            }
        };

        let operation_id = self.catalog.as_ref().and_then(|catalog| {
            catalog
                .signals
                .get(&signal.to_lowercase())
                .map(SignalDeclaration::operation_id)
        });
        if operation_id.is_none()
            && self
                .catalog
                .as_ref()
                .is_some_and(|catalog| !catalog.signals.is_empty())
        {
            let catalog = self.catalog.as_ref().unwrap();
            return Err(unknown(
                "signal",
                &signal,
                catalog.signals.values().map(SignalDeclaration::name),
            ));
        }
        let context = self
            .context
            .as_deref_mut()
            .ok_or_else(|| EvalError::Runtime("signal callbacks require a session".into()))?;
        let callback = context.allocate_callback();
        self.callbacks.insert(callback, function);
        let request = match operation_id {
            Some(operation_id) => HostRequest::RegisterSignalHandler {
                operation_id,
                scope,
                name,
                callback,
            },
            None => HostRequest::RegisterSignalHandlerByName {
                scope,
                signal,
                name,
                callback,
            },
        };
        match self.dispatch(request) {
            Ok(HostResponse::Unit) => {
                self.context
                    .as_deref_mut()
                    .unwrap()
                    .event_bindings
                    .insert(callback, crate::host::EventBindingState { bypassed: false });
                Ok(Value::EventBinding(callback))
            }
            Err(error) => {
                self.callbacks.remove(&callback);
                Err(error)
            }
            other => {
                self.callbacks.remove(&callback);
                Err(EvalError::Runtime(format!(
                    "signal registration returned unexpected response {other:?}"
                )))
            }
        }
    }

    pub(super) fn event_binding_method(
        &mut self,
        callback: CallbackHandle,
        method: &str,
        args: &[Value],
    ) -> Result<Value, EvalError> {
        let context = self
            .context
            .as_deref()
            .ok_or_else(|| EvalError::Runtime("event binding methods require a session".into()))?;
        if !context.owns_callback(callback) {
            return Err(HostError {
                kind: HostErrorKind::ForeignHandle,
                operation: method.into(),
                message: "event binding belongs to another session".into(),
            }
            .into());
        }
        let retained = context.event_bindings.get(&callback).copied();
        let request = match (method, args) {
            ("remove", []) => {
                if retained.is_none() {
                    return Ok(Value::Null);
                }
                HostRequest::RemoveEventBinding { callback }
            }
            ("bypass", [Value::Bool(bypassed)]) => {
                let state = retained.ok_or_else(|| HostError {
                    kind: HostErrorKind::StaleHandle,
                    operation: "bypass".into(),
                    message: "event binding has been removed".into(),
                })?;
                if state.bypassed == *bypassed {
                    return Ok(Value::Null);
                }
                HostRequest::SetEventBindingBypass {
                    callback,
                    bypassed: *bypassed,
                }
            }
            ("remove", _) => {
                return Err(EvalError::Runtime(
                    "binding.remove expects no arguments".into(),
                ));
            }
            ("bypass", _) => {
                return Err(EvalError::Runtime(
                    "binding.bypass expects one boolean".into(),
                ));
            }
            _ => {
                return Err(EvalError::Runtime(format!(
                    "unknown event binding method '{method}'"
                )));
            }
        };
        match self.dispatch(request)? {
            HostResponse::Unit => {}
            other => {
                return Err(EvalError::Runtime(format!(
                    "event binding operation returned unexpected response {other:?}"
                )));
            }
        }
        if method == "remove" {
            self.context
                .as_deref_mut()
                .unwrap()
                .event_bindings
                .remove(&callback);
            self.callbacks.remove(&callback);
        } else if let Value::Bool(bypassed) = args[0] {
            self.context
                .as_deref_mut()
                .unwrap()
                .event_bindings
                .get_mut(&callback)
                .unwrap()
                .bypassed = bypassed;
        }
        Ok(Value::Null)
    }
}
