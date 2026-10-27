//! Small general-purpose hosts used by examples and integration tests.

use std::io::Write;

use crate::{
    CallbackHandle, ComponentHandle, Host, HostContext, HostError, HostErrorKind, HostRequest,
    HostResponse, MaterializedCE, OperationId, TransportValue,
};

#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Emit {
        handle: ComponentHandle,
        tree: MaterializedCE,
    },
    Register {
        handle: ComponentHandle,
        tree: MaterializedCE,
    },
    Attach {
        parent: Option<ComponentHandle>,
        child: ComponentHandle,
    },
    MethodById {
        component: ComponentHandle,
        operation_id: OperationId,
        args: Vec<crate::Value>,
    },
    Method {
        component: ComponentHandle,
        component_type: String,
        method: String,
        args: Vec<crate::Value>,
    },
    SignalById {
        operation_id: OperationId,
        scope: Option<ComponentHandle>,
        name: Option<String>,
        callback: CallbackHandle,
    },
    Signal {
        signal: String,
        scope: Option<ComponentHandle>,
        name: Option<String>,
        callback: CallbackHandle,
    },
    Api {
        id: String,
        args: Vec<TransportValue>,
    },
    ApiById {
        operation_id: OperationId,
        args: Vec<TransportValue>,
    },
}

/// An ordered in-memory event stream suitable for forwarding to a socket or
/// message broker by an embedding application.
#[derive(Debug, Clone)]
pub struct EventStreamHost {
    pub events: Vec<HostEvent>,
}

impl EventStreamHost {
    pub fn new() -> Self {
        Self { events: vec![] }
    }
}

impl Default for EventStreamHost {
    fn default() -> Self {
        Self::new()
    }
}

impl Host for EventStreamHost {
    fn dispatch_with_context(
        &mut self,
        context: &mut HostContext,
        request: HostRequest,
    ) -> Result<HostResponse, HostError> {
        let event = match request {
            HostRequest::ValidateComponents { components } => {
                let mut live = Vec::new();
                for component in components {
                    require_handle(context, component, "validate_components")?;
                    if let Some(tree) = self.events.iter().find_map(|event| match event {
                        HostEvent::Emit { handle, tree } | HostEvent::Register { handle, tree }
                            if *handle == component =>
                        {
                            Some(tree)
                        }
                        _ => None,
                    }) {
                        live.push((component, tree.component_type.clone()));
                    }
                }
                return Ok(HostResponse::Components(live));
            }
            HostRequest::Emit { tree } => {
                let handle = context.allocate_component();
                HostEvent::Emit { handle, tree }
            }
            HostRequest::RegisterComponent { tree } => {
                let handle = context.allocate_component();
                HostEvent::Register { handle, tree }
            }
            HostRequest::Attach { parent, child } => {
                require_handle(context, child, "attach")?;
                if let Some(parent) = parent {
                    require_handle(context, parent, "attach")?;
                }
                HostEvent::Attach { parent, child }
            }
            HostRequest::InvokeComponentMethod {
                operation_id,
                component,
                args,
            } => {
                require_handle(context, component, "invoke_component_method")?;
                HostEvent::MethodById {
                    component,
                    operation_id,
                    args,
                }
            }
            HostRequest::InvokeComponentMethodByName {
                component,
                component_type,
                method,
                args,
            } => {
                require_handle(context, component, "invoke_component_method")?;
                HostEvent::Method {
                    component,
                    component_type,
                    method,
                    args,
                }
            }
            HostRequest::RegisterSignalHandler {
                operation_id,
                scope,
                name,
                callback,
            } => {
                if let Some(scope) = scope {
                    require_handle(context, scope, "register_signal_handler")?;
                }
                require_callback(context, callback, "register_signal_handler")?;
                HostEvent::SignalById {
                    operation_id,
                    scope,
                    name,
                    callback,
                }
            }
            HostRequest::RegisterSignalHandlerByName {
                signal,
                scope,
                name,
                callback,
            } => {
                if let Some(scope) = scope {
                    require_handle(context, scope, "register_signal_handler_by_name")?;
                }
                require_callback(context, callback, "register_signal_handler_by_name")?;
                HostEvent::Signal {
                    signal,
                    scope,
                    name,
                    callback,
                }
            }
            HostRequest::CallApi { api_id, args } => HostEvent::Api { id: api_id, args },
            HostRequest::CallApiById { operation_id, args } => {
                HostEvent::ApiById { operation_id, args }
            }
            other => return Err(HostError::unsupported(other.operation_name())),
        };
        let response = match &event {
            HostEvent::Emit { handle, tree } | HostEvent::Register { handle, tree } => {
                HostResponse::Component {
                    handle: *handle,
                    component_type: tree.component_type.clone(),
                }
            }
            _ => HostResponse::Unit,
        };
        self.events.push(event);
        Ok(response)
    }
}

/// JSON-lines recorder with the same semantics as `EventStreamHost`.
pub struct JsonLinesHost<W: Write> {
    inner: EventStreamHost,
    writer: W,
}

impl<W: Write> JsonLinesHost<W> {
    pub fn new(writer: W) -> Self {
        Self {
            inner: EventStreamHost::new(),
            writer,
        }
    }
    pub fn into_inner(self) -> W {
        self.writer
    }
    pub fn into_inner_ref(&self) -> &W {
        &self.writer
    }
}

impl<W: Write> Host for JsonLinesHost<W> {
    fn dispatch_with_context(
        &mut self,
        context: &mut HostContext,
        request: HostRequest,
    ) -> Result<HostResponse, HostError> {
        // Liveness inspection records no effect and must not repeat the last
        // emitted event in the JSON-lines stream.
        if matches!(&request, HostRequest::ValidateComponents { .. }) {
            return self.inner.dispatch_with_context(context, request);
        }
        let response = self.inner.dispatch_with_context(context, request)?;
        let event = self
            .inner
            .events
            .last()
            .expect("successful dispatch records an event");
        let line = event_json(event);
        writeln!(self.writer, "{line}")
            .map_err(|error| HostError::failure("json_lines", error.to_string()))?;
        Ok(response)
    }
}

fn require_handle(
    context: &HostContext,
    handle: ComponentHandle,
    operation: &str,
) -> Result<(), HostError> {
    if context.owns_component(handle) {
        Ok(())
    } else {
        Err(HostError {
            kind: HostErrorKind::ForeignHandle,
            operation: operation.into(),
            message: format!("component handle {handle:?} is stale or foreign"),
        })
    }
}

fn require_callback(
    context: &HostContext,
    handle: CallbackHandle,
    operation: &str,
) -> Result<(), HostError> {
    if context.owns_callback(handle) {
        Ok(())
    } else {
        Err(HostError {
            kind: HostErrorKind::ForeignHandle,
            operation: operation.into(),
            message: format!("callback handle {handle:?} is stale or foreign"),
        })
    }
}

fn event_json(event: &HostEvent) -> String {
    let (operation, handle, detail) = match event {
        HostEvent::Emit { handle, tree } => (
            "emit",
            Some(*handle),
            format!("component={}", tree.component_type),
        ),
        HostEvent::Register { handle, tree } => (
            "register",
            Some(*handle),
            format!("component={}", tree.component_type),
        ),
        HostEvent::Attach { child, parent } => {
            ("attach", Some(*child), format!("parent={parent:?}"))
        }
        HostEvent::MethodById {
            component,
            operation_id,
            ..
        } => (
            "method",
            Some(*component),
            format!("operation_id={operation_id:?}"),
        ),
        HostEvent::Method {
            component,
            component_type,
            method,
            ..
        } => (
            "method",
            Some(*component),
            format!("component={component_type};method={method}"),
        ),
        HostEvent::SignalById {
            operation_id,
            scope,
            callback,
            ..
        } => (
            "signal",
            *scope,
            format!("operation_id={operation_id:?};callback={callback:?}"),
        ),
        HostEvent::Signal {
            signal,
            scope,
            callback,
            ..
        } => (
            "signal",
            *scope,
            format!("signal={signal};callback={callback:?}"),
        ),
        HostEvent::Api { id, .. } => ("api", None, format!("id={id}")),
        HostEvent::ApiById { operation_id, .. } => {
            ("api", None, format!("operation_id={operation_id:?}"))
        }
    };
    format!(
        "{{\"operation\":\"{}\",\"handle\":{},\"detail\":\"{}\"}}",
        escape(operation),
        handle.map_or_else(|| "null".into(), |h| h.into_raw().to_string()),
        escape(&detail)
    )
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
