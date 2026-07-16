//! Small general-purpose hosts used by examples and integration tests.

use std::io::Write;

use crate::{ComponentHandle, Host, HostCapabilities, HostContext, HostError,
    HostErrorKind, HostRequest, HostResponse, MaterializedCE, TransportValue};

#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Emit { handle: ComponentHandle, tree: MaterializedCE },
    Register { handle: ComponentHandle, tree: MaterializedCE },
    Attach { parent: Option<ComponentHandle>, child: ComponentHandle },
    Method { component: ComponentHandle, component_type: String, method: String, args: Vec<crate::Value> },
    Api { id: String, args: Vec<TransportValue> },
}

/// An ordered in-memory event stream suitable for forwarding to a socket or
/// message broker by an embedding application.
#[derive(Debug, Clone)]
pub struct EventStreamHost {
    capabilities: HostCapabilities,
    pub events: Vec<HostEvent>,
}

impl EventStreamHost {
    pub fn new(capabilities: HostCapabilities) -> Self { Self { capabilities, events: vec![] } }
}

impl Host for EventStreamHost {
    fn capabilities(&self) -> HostCapabilities { self.capabilities.clone() }
    fn dispatch_with_context(&mut self, context: &mut HostContext, request: HostRequest) -> Result<HostResponse, HostError> {
        let event = match request {
            HostRequest::Emit { tree } => {
                let handle = context.allocate_component();
                HostEvent::Emit { handle, tree }
            }
            HostRequest::RegisterComponent { tree } => {
                let handle = context.allocate_component();
                HostEvent::Register { handle, tree }
            }
            HostRequest::Attach { parent, child } => {
                require_handle(context, child, "attach")?; if let Some(parent) = parent { require_handle(context, parent, "attach")?; }
                HostEvent::Attach { parent, child }
            }
            HostRequest::InvokeComponentMethod { component, component_type, method, args } => {
                require_handle(context, component, "invoke_component_method")?;
                HostEvent::Method { component, component_type, method, args }
            }
            HostRequest::CallApi { api_id, args } => HostEvent::Api { id: api_id, args },
            other => return Err(HostError::unsupported(other.operation_name())),
        };
        let response = match &event {
            HostEvent::Emit { handle, tree } | HostEvent::Register { handle, tree } => HostResponse::Component { handle: *handle, component_type: tree.component_type.clone() },
            _ => HostResponse::Unit,
        };
        self.events.push(event); Ok(response)
    }
}

/// JSON-lines recorder with the same semantics as `EventStreamHost`.
pub struct JsonLinesHost<W: Write> {
    inner: EventStreamHost,
    writer: W,
}

impl<W: Write> JsonLinesHost<W> {
    pub fn new(writer: W, capabilities: HostCapabilities) -> Self { Self { inner: EventStreamHost::new(capabilities), writer } }
    pub fn into_inner(self) -> W { self.writer }
    pub fn into_inner_ref(&self) -> &W { &self.writer }
}

impl<W: Write> Host for JsonLinesHost<W> {
    fn capabilities(&self) -> HostCapabilities { self.inner.capabilities() }
    fn dispatch_with_context(&mut self, context: &mut HostContext, request: HostRequest) -> Result<HostResponse, HostError> {
        let response = self.inner.dispatch_with_context(context, request)?;
        let event = self.inner.events.last().expect("successful dispatch records an event");
        let line = event_json(event);
        writeln!(self.writer, "{line}").map_err(|error| HostError::failure("json_lines", error.to_string()))?;
        Ok(response)
    }
}

fn require_handle(context: &HostContext, handle: ComponentHandle, operation: &str) -> Result<(), HostError> {
    if context.owns_component(handle) { Ok(()) } else { Err(HostError { kind: HostErrorKind::ForeignHandle,
        operation: operation.into(), message: format!("component handle {handle:?} is stale or foreign") }) }
}

fn event_json(event: &HostEvent) -> String {
    let (operation, handle, detail) = match event {
        HostEvent::Emit { handle, tree } => ("emit", Some(*handle), format!("component={}", tree.component_type)),
        HostEvent::Register { handle, tree } => ("register", Some(*handle), format!("component={}", tree.component_type)),
        HostEvent::Attach { child, parent } => ("attach", Some(*child), format!("parent={parent:?}")),
        HostEvent::Method { component, component_type, method, .. } => ("method", Some(*component), format!("component={component_type};method={method}")),
        HostEvent::Api { id, .. } => ("api", None, format!("id={id}")),
    };
    format!("{{\"operation\":\"{}\",\"handle\":{},\"detail\":\"{}\"}}",
        escape(operation), handle.map_or_else(|| "null".into(), |h| h.into_raw().to_string()), escape(&detail))
}

fn escape(value: &str) -> String { value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n") }
