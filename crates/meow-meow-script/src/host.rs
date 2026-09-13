use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::object::{MaterializedCE, Value};

/// An opaque component identity owned by the scripting boundary.
///
/// Hosts may encode their native generational handle losslessly in this value,
/// but scripts cannot inspect or manufacture its representation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentHandle(u64);

impl ComponentHandle {
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn into_raw(self) -> u64 {
        self.0
    }
}

/// Opaque identity for a script closure retained by a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CallbackHandle(u64);

impl CallbackHandle {
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
    pub fn into_raw(self) -> u64 {
        self.0
    }
}

/// Opaque identity for one retained MMS session.
///
/// Callback handles are deliberately session-local.  Carrying this identity
/// beside a callback lets a host reject a callback that belongs to a different
/// (or already closed) session without inspecting the callback representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionHandle(u32);

impl SessionHandle {
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    pub fn into_raw(self) -> u32 {
        self.0
    }
}

/// A host-storable reference to a closure retained by its originating MMS
/// session.  It contains no body, captured environment, or heap state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionCallbackRef {
    pub session: SessionHandle,
    pub callback: CallbackHandle,
}

/// Values that are safe to own outside the MMS heap. In particular, tables
/// are snapshots and closures are represented only by opaque handles.
#[derive(Debug, Clone, PartialEq)]
pub enum TransportValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<TransportValue>),
    Table(Vec<(String, TransportValue)>),
    Component(ComponentHandle),
    Callback(CallbackHandle),
}

/// A transport-safe request to invoke an MMS-owned callback on its session.
/// Hosts may queue this value, but only the originating session can execute it.
#[derive(Debug, Clone, PartialEq)]
pub struct CallbackInvocation {
    pub callback: CallbackHandle,
    pub args: Vec<TransportValue>,
}

/// Stable identity of a source unit as resolved by its host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(String);

impl SourceId {
    pub fn new(identity: impl Into<String>) -> Self {
        Self(identity.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSource {
    pub identity: SourceId,
    pub source: String,
}

/// Allocator and ownership checker supplied to host dispatch.
///
/// Component handles identify host-owned resources, so effectful hosts may
/// return handles derived from native IDs. Simple hosts can use
/// `allocate_component` for synthetic identities. Callback handles identify
/// MMS-owned closures and are always allocated by the runtime.
#[derive(Debug)]
pub struct HostContext {
    session_tag: u32,
    next_component: u32,
    next_callback: u32,
    components: HashSet<ComponentHandle>,
    callbacks: HashSet<CallbackHandle>,
}

impl HostContext {
    pub(crate) fn new(session_tag: u32) -> Self {
        Self {
            session_tag,
            next_component: 1,
            next_callback: 1,
            components: HashSet::new(),
            callbacks: HashSet::new(),
        }
    }
    pub fn allocate_component(&mut self) -> ComponentHandle {
        let handle = ComponentHandle::from_raw(
            ((self.session_tag as u64) << 32) | self.next_component as u64,
        );
        self.next_component = self
            .next_component
            .checked_add(1)
            .expect("component handle space exhausted");
        self.components.insert(handle);
        handle
    }
    pub fn adopt_component(&mut self, handle: ComponentHandle) {
        self.components.insert(handle);
    }
    pub fn allocate_callback(&mut self) -> CallbackHandle {
        let handle =
            CallbackHandle::from_raw(((self.session_tag as u64) << 32) | self.next_callback as u64);
        self.next_callback = self
            .next_callback
            .checked_add(1)
            .expect("callback handle space exhausted");
        self.callbacks.insert(handle);
        handle
    }
    pub fn owns_component(&self, handle: ComponentHandle) -> bool {
        self.components.contains(&handle)
    }
    pub fn owns_callback(&self, handle: CallbackHandle) -> bool {
        self.callbacks.contains(&handle)
    }
    pub fn session_handle(&self) -> SessionHandle {
        SessionHandle::from_raw(self.session_tag)
    }
}

impl fmt::Debug for ComponentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ComponentHandle").field(&self.0).finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostRequest {
    /// Register an uninitialized component. The host returns its component
    /// identity in `HostResponse::Component`.
    RegisterComponent {
        tree: MaterializedCE,
    },
    /// Emit a component tree. The host returns its component identity in
    /// `HostResponse::Component`.
    Emit {
        tree: MaterializedCE,
    },
    Spawn {
        tree: MaterializedCE,
    },
    Register {
        tree: MaterializedCE,
    },
    Attach {
        parent: Option<ComponentHandle>,
        child: ComponentHandle,
    },
    Query {
        selector: String,
        scope: Option<ComponentHandle>,
        multiple: bool,
    },
    /// Register an MMS-owned callback for a configured signal. A signal scope
    /// is optional; `None` denotes global registration.
    RegisterSignalHandler {
        operation_id: crate::OperationId,
        scope: Option<ComponentHandle>,
        name: Option<String>,
        callback: CallbackHandle,
    },
    /// Compatibility request for open/legacy runtimes without configured
    /// signal operation identities.
    RegisterSignalHandlerByName {
        scope: Option<ComponentHandle>,
        signal: String,
        name: Option<String>,
        callback: CallbackHandle,
    },
    /// Invoke a configured method on a checked live component receiver. The
    /// RuntimeSpec has already validated the method and arguments; the host
    /// selects its implementation exclusively through `operation_id`.
    InvokeComponentMethod {
        operation_id: crate::OperationId,
        component: ComponentHandle,
        args: Vec<Value>,
    },
    /// Compatibility request for open and legacy runtimes whose component
    /// method declaration has no configured host operation identity.
    InvokeComponentMethodByName {
        component: ComponentHandle,
        component_type: String,
        method: String,
        args: Vec<Value>,
    },
    CallApi {
        api_id: String,
        args: Vec<TransportValue>,
    },
    /// A configured API resolved by `RuntimeSpec` before host dispatch.
    CallApiById {
        operation_id: crate::OperationId,
        args: Vec<TransportValue>,
    },
    LoadSource {
        importer: Option<SourceId>,
        specifier: String,
    },
}

impl HostRequest {
    pub fn operation_name(&self) -> &str {
        match self {
            Self::RegisterComponent { .. } => "register_component",
            Self::Emit { .. } => "emit",
            Self::Spawn { .. } => "spawn",
            Self::Register { .. } => "register",
            Self::Attach { .. } => "attach",
            Self::Query { .. } => "query",
            Self::RegisterSignalHandler { .. } => "register_signal_handler",
            Self::RegisterSignalHandlerByName { .. } => "register_signal_handler_by_name",
            Self::InvokeComponentMethod { .. } => "invoke_component_method",
            Self::InvokeComponentMethodByName { .. } => "invoke_component_method_by_name",
            Self::CallApi { api_id, .. } => api_id,
            Self::CallApiById { .. } => "call_api_by_id",
            Self::LoadSource { .. } => "load_source",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostResponse {
    Unit,
    Value(Value),
    Component {
        handle: ComponentHandle,
        component_type: String,
    },
    Components(Vec<(ComponentHandle, String)>),
    Transport(TransportValue),
    Source(LoadedSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostErrorKind {
    UnsupportedHostOperation,
    UnavailableContext,
    InvalidRequest,
    HostFailure,
    ForeignHandle,
    StaleHandle,
    Conversion,
    SourceFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError {
    pub kind: HostErrorKind,
    pub operation: String,
    pub message: String,
}

impl HostError {
    pub fn unsupported(operation: impl Into<String>) -> Self {
        let operation = operation.into();
        Self {
            kind: HostErrorKind::UnsupportedHostOperation,
            message: format!("host operation '{operation}' is unavailable"),
            operation,
        }
    }

    /// The operation is part of the configured runtime, but the short-lived
    /// host servicing this call does not currently have the required service.
    pub fn unavailable(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: HostErrorKind::UnavailableContext,
            operation: operation.into(),
            message: message.into(),
        }
    }

    pub fn failure(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: HostErrorKind::HostFailure,
            operation: operation.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.message)
    }
}

impl Error for HostError {}

/// Synchronous interface through which evaluation requests host capabilities.
pub trait Host {
    /// Legacy dispatch entry point. New hosts may implement only
    /// `dispatch_with_context` and leave this default in place.
    fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
        Err(HostError::unsupported(request.operation_name()))
    }

    fn dispatch_with_context(
        &mut self,
        _context: &mut HostContext,
        request: HostRequest,
    ) -> Result<HostResponse, HostError> {
        self.dispatch(request)
    }
}

/// Capability-free host used for deterministic pure-language evaluation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hostless;

impl Host for Hostless {
    fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
        Err(HostError::unsupported(request.operation_name()))
    }
}
