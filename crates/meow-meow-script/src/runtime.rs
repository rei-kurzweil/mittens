//! Configurable MMS runtime and persistent sessions.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::{CallbackHandle, EvalError, Evaluation, Evaluator, Expression, HeapHandle, Host,
    HostCapabilities, HostContext, Hostless, MaterializedCE, MeowMeowParser, MeowMeowTokenizer,
    Statement, Value};

static NEXT_SESSION_TAG: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Any, Null, Bool, Number, String, Array, Table, Component, Callback,
}

impl ValueType {
    pub fn accepts(&self, value: &Value) -> bool {
        matches!(self, Self::Any)
            || matches!((self, value),
                (Self::Null, Value::Null) | (Self::Bool, Value::Bool(_))
                | (Self::Number, Value::Number(_)) | (Self::String, Value::String(_))
                | (Self::Array, Value::Array(_)) | (Self::Table, Value::Map(_) | Value::Object(_))
                | (Self::Component, Value::ComponentObject { .. })
                | (Self::Callback, Value::Function { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueSignature {
    pub arguments: Vec<ValueType>,
    pub result: ValueType,
    pub variadic: bool,
}

impl ValueSignature {
    pub fn new(arguments: impl Into<Vec<ValueType>>, result: ValueType) -> Self {
        Self { arguments: arguments.into(), result, variadic: false }
    }
    pub fn any() -> Self { Self { arguments: vec![], result: ValueType::Any, variadic: true } }
}

pub type ComponentCallback = Arc<dyn Fn(&mut MaterializedCE) -> Result<(), String> + Send + Sync>;

#[derive(Clone)]
pub struct ComponentSpec {
    pub name: String,
    pub aliases: Vec<String>,
    pub constructors: HashMap<String, ValueSignature>,
    pub builder_calls: HashMap<String, ValueSignature>,
    pub properties: HashMap<String, ValueType>,
    pub positional: Vec<ValueType>,
    pub methods: HashMap<String, ValueSignature>,
    pub required_capability: Option<String>,
    pub normalize: Option<ComponentCallback>,
    pub validate: Option<ComponentCallback>,
}

impl fmt::Debug for ComponentSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentSpec").field("name", &self.name).field("aliases", &self.aliases).finish_non_exhaustive()
    }
}

impl ComponentSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), aliases: vec![], constructors: HashMap::new(),
            builder_calls: HashMap::new(), properties: HashMap::new(), positional: vec![],
            methods: HashMap::new(), required_capability: None, normalize: None, validate: None }
    }
    pub fn alias(mut self, alias: impl Into<String>) -> Self { self.aliases.push(alias.into()); self }
    pub fn constructor(mut self, name: impl Into<String>, signature: ValueSignature) -> Self { self.constructors.insert(name.into(), signature); self }
    pub fn builder_call(mut self, name: impl Into<String>, signature: ValueSignature) -> Self { self.builder_calls.insert(name.into(), signature); self }
    pub fn property(mut self, name: impl Into<String>, ty: ValueType) -> Self { self.properties.insert(name.into(), ty); self }
    pub fn positional(mut self, ty: ValueType) -> Self { self.positional.push(ty); self }
    pub fn method(mut self, name: impl Into<String>, signature: ValueSignature) -> Self { self.methods.insert(name.into(), signature); self }
    pub fn requires(mut self, capability: impl Into<String>) -> Self { self.required_capability = Some(capability.into()); self }
    pub fn normalize_with(mut self, callback: impl Fn(&mut MaterializedCE) -> Result<(), String> + Send + Sync + 'static) -> Self { self.normalize = Some(Arc::new(callback)); self }
    pub fn validate_with(mut self, callback: impl Fn(&mut MaterializedCE) -> Result<(), String> + Send + Sync + 'static) -> Self { self.validate = Some(Arc::new(callback)); self }
}

#[derive(Debug, Clone)]
pub struct HostApiSpec {
    pub id: String,
    pub namespace: Option<String>,
    pub name: String,
    pub signature: ValueSignature,
    pub required_capability: String,
}

impl HostApiSpec {
    pub fn function(name: impl Into<String>, signature: ValueSignature) -> Self {
        let name = name.into(); Self { id: name.clone(), namespace: None, name,
            signature, required_capability: String::new() }
    }
    pub fn method(namespace: impl Into<String>, name: impl Into<String>, signature: ValueSignature) -> Self {
        let namespace = namespace.into(); let name = name.into();
        Self { id: format!("{namespace}.{name}"), namespace: Some(namespace), name, signature,
            required_capability: String::new() }
    }
    pub fn id(mut self, id: impl Into<String>) -> Self { self.id = id.into(); self }
    pub fn requires(mut self, capability: impl Into<String>) -> Self { self.required_capability = capability.into(); self }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogErrorKind { DuplicateName, NameConflict, CapabilityMismatch }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError { pub kind: CatalogErrorKind, pub name: String, pub message: String }
impl fmt::Display for CatalogError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.message) } }
impl std::error::Error for CatalogError {}

#[derive(Debug, Clone)]
pub(crate) struct Catalog {
    pub components: HashMap<String, Arc<ComponentSpec>>,
    pub canonical_components: Vec<Arc<ComponentSpec>>,
    pub apis: HashMap<String, Arc<HostApiSpec>>,
    pub namespaces: HashSet<String>,
    pub builtins: HashSet<String>,
}

impl Catalog {
    pub(crate) fn has_namespace(&self, name: &str) -> bool {
        self.namespaces.iter().any(|namespace| namespace.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeBuilder { catalog: Catalog }

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self { catalog: Catalog { components: HashMap::new(), canonical_components: vec![], apis: HashMap::new(),
            namespaces: HashSet::new(), builtins: ["null", "range", "len", "query", "query_all", "Math", "MusicNote"].into_iter().map(str::to_owned).collect() } }
    }
}

impl RuntimeBuilder {
    pub fn new() -> Self { Self::default() }
    pub fn register_builtin(&mut self, name: impl Into<String>) -> Result<&mut Self, CatalogError> {
        let name = name.into(); self.ensure_free(&name)?; self.catalog.builtins.insert(name); Ok(self)
    }
    pub fn register_component(&mut self, spec: ComponentSpec) -> Result<&mut Self, CatalogError> {
        let spec = Arc::new(spec);
        for name in std::iter::once(&spec.name).chain(&spec.aliases) { self.ensure_free(name)?; }
        for name in std::iter::once(&spec.name).chain(&spec.aliases) {
            self.catalog.components.insert(name.to_lowercase(), spec.clone());
        }
        self.catalog.canonical_components.push(spec); Ok(self)
    }
    pub fn register_host_api(&mut self, spec: HostApiSpec) -> Result<&mut Self, CatalogError> {
        if let Some(namespace) = &spec.namespace {
            if !self.catalog.namespaces.contains(namespace) { self.ensure_free(namespace)?; }
            self.catalog.namespaces.insert(namespace.clone());
        } else { self.ensure_free(&spec.name)?; }
        let key = api_key(spec.namespace.as_deref(), &spec.name);
        if self.catalog.apis.contains_key(&key) { return Err(duplicate(&key)); }
        self.catalog.apis.insert(key, Arc::new(spec)); Ok(self)
    }
    fn ensure_free(&self, name: &str) -> Result<(), CatalogError> {
        let lower = name.to_lowercase();
        if self.catalog.components.contains_key(&lower) || self.catalog.builtins.iter().any(|v| v.eq_ignore_ascii_case(name))
            || self.catalog.namespaces.iter().any(|v| v.eq_ignore_ascii_case(name))
            || self.catalog.apis.values().any(|api| api.namespace.is_none() && api.name.eq_ignore_ascii_case(name)) {
            return Err(duplicate(name));
        } Ok(())
    }
    pub fn build(self) -> Runtime { Runtime { catalog: Arc::new(self.catalog) } }
}

fn duplicate(name: &str) -> CatalogError { CatalogError { kind: CatalogErrorKind::DuplicateName, name: name.into(), message: format!("catalog name '{name}' is already registered") } }
pub(crate) fn api_key(namespace: Option<&str>, name: &str) -> String { namespace.map_or_else(|| name.to_lowercase(), |ns| format!("{}.{}", ns.to_lowercase(), name.to_lowercase())) }

#[derive(Debug, Clone)]
pub struct Runtime { pub(crate) catalog: Arc<Catalog> }
impl Runtime {
    pub fn builder() -> RuntimeBuilder { RuntimeBuilder::new() }
    pub fn component_names(&self) -> impl Iterator<Item = &str> { self.catalog.components.keys().map(String::as_str) }
    pub fn materialize_component(&self, source: &str) -> Result<MaterializedCE, EvalError> {
        let tokens = MeowMeowTokenizer::new(source)
            .tokenize()
            .map_err(|e| EvalError::Tokenize(format!("{e:?}")))?;
        let statements = MeowMeowParser::with_component_names(tokens, self.catalog.components.keys().cloned())
            .parse_program()
            .map_err(|e| EvalError::Parse(e.message))?;
        let [Statement::Expression(Expression::Component(component))] = statements.as_slice() else {
            return Err(EvalError::Runtime("expected exactly one component expression".into()));
        };
        let mut host = Hostless;
        let tag = NEXT_SESSION_TAG.fetch_add(1, Ordering::Relaxed);
        let mut context = HostContext::new(tag);
        let mut evaluator = Evaluator::for_session(
            &mut host,
            vec![HashMap::from([("null".into(), Value::Null)])],
            HeapHandle::new(),
            HashMap::new(),
            self.catalog.clone(),
            &mut context,
        );
        evaluator.materialize(component)
    }
    pub fn session<H: Host>(&self, host: H) -> Result<Session<H>, CatalogError> {
        check_capabilities(&self.catalog, &host.capabilities())?;
        let tag = NEXT_SESSION_TAG.fetch_add(1, Ordering::Relaxed);
        Ok(Session { runtime: self.clone(), host, scopes: vec![HashMap::from([("null".into(), Value::Null)])],
            heap: HeapHandle::new(), callbacks: HashMap::new(), context: HostContext::new(tag) })
    }
}

fn check_capabilities(catalog: &Catalog, host: &HostCapabilities) -> Result<(), CatalogError> {
    for component in &catalog.canonical_components {
        if !host.components.contains(&component.name.to_lowercase()) {
            return Err(CatalogError { kind: CatalogErrorKind::CapabilityMismatch, name: component.name.clone(),
                message: format!("host does not support component '{}'", component.name) });
        }
        if let Some(capability) = &component.required_capability {
            if !host.component_operations.contains(capability) { return Err(CatalogError { kind: CatalogErrorKind::CapabilityMismatch,
                name: capability.clone(), message: format!("host is missing component capability '{capability}'") }); }
        }
    }
    for api in catalog.apis.values() {
        let required = if api.required_capability.is_empty() { &api.id } else { &api.required_capability };
        if !host.api_ids.contains(required) { return Err(CatalogError { kind: CatalogErrorKind::CapabilityMismatch,
            name: required.clone(), message: format!("host is missing API capability '{required}'") }); }
    }
    Ok(())
}

pub struct Session<H: Host> {
    runtime: Runtime,
    host: H,
    pub(crate) scopes: Vec<HashMap<String, Value>>,
    pub(crate) heap: HeapHandle,
    pub(crate) callbacks: HashMap<CallbackHandle, Value>,
    pub(crate) context: HostContext,
}

impl<H: Host> Session<H> {
    pub fn eval(&mut self, source: &str) -> Result<Evaluation, EvalError> {
        let scopes = std::mem::take(&mut self.scopes);
        let callbacks = std::mem::take(&mut self.callbacks);
        let mut evaluator = Evaluator::for_session(&mut self.host, scopes, self.heap.clone(), callbacks,
            self.runtime.catalog.clone(), &mut self.context);
        let result = evaluator.evaluate(source);
        let (scopes, callbacks) = evaluator.into_session_state();
        self.scopes = scopes; self.callbacks = callbacks; result
    }
    pub fn invoke_callback(&mut self, handle: CallbackHandle, args: Vec<Value>) -> Result<Value, EvalError> {
        if !self.context.owns_callback(handle) { return Err(EvalError::Runtime(format!("stale or foreign callback {handle:?}"))); }
        let callback = self.callbacks.get(&handle).cloned().ok_or_else(|| EvalError::Runtime(format!("unknown callback {handle:?}")))?;
        let scopes = std::mem::take(&mut self.scopes); let callbacks = std::mem::take(&mut self.callbacks);
        let mut evaluator = Evaluator::for_session(&mut self.host, scopes, self.heap.clone(), callbacks,
            self.runtime.catalog.clone(), &mut self.context);
        let result = evaluator.invoke_value(callback, args);
        let (scopes, callbacks) = evaluator.into_session_state(); self.scopes = scopes; self.callbacks = callbacks; result
    }
    pub fn host(&self) -> &H { &self.host }
    pub fn host_mut(&mut self) -> &mut H { &mut self.host }
    pub fn context(&self) -> &HostContext { &self.context }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventStreamHost, HostError, HostRequest, HostResponse, TransportValue};

    struct FixedHandleHost {
        capabilities: HostCapabilities,
        handle: crate::ComponentHandle,
        requests: Vec<HostRequest>,
    }

    impl crate::Host for FixedHandleHost {
        fn capabilities(&self) -> HostCapabilities { self.capabilities.clone() }
        fn dispatch_with_context(&mut self, _context: &mut HostContext, request: HostRequest) -> Result<HostResponse, HostError> {
            self.requests.push(request.clone());
            match request {
                HostRequest::Emit { tree } | HostRequest::RegisterComponent { tree } => {
                    Ok(HostResponse::Component { handle: self.handle, component_type: tree.component_type })
                }
                HostRequest::InvokeComponentMethod { .. } => Ok(HostResponse::Unit),
                other => Err(HostError::unsupported(other.operation_name())),
            }
        }
    }

    fn runtime() -> Runtime {
        let mut builder = Runtime::builder();
        builder.register_component(ComponentSpec::new("Panel").alias("panel")
            .constructor("new", ValueSignature::new(vec![ValueType::Number], ValueType::Component))
            .property("title", ValueType::String)
            .method("show", ValueSignature::new(vec![], ValueType::Null))
            .normalize_with(|tree| { tree.component_type = "Panel".into(); Ok(()) })).unwrap();
        builder.register_host_api(HostApiSpec::method("log", "write",
            ValueSignature::new(vec![ValueType::String], ValueType::Null)).requires("log.write")).unwrap();
        builder.register_host_api(HostApiSpec::method("sink", "write",
            ValueSignature::new(vec![ValueType::Any], ValueType::Null)).requires("sink.write")).unwrap();
        builder.build()
    }

    fn capabilities() -> HostCapabilities {
        HostCapabilities::default().supports_component("Panel").supports_api("log.write").supports_api("sink.write")
    }

    #[test]
    fn catalog_parses_lowercase_aliases_and_issues_handles() {
        let mut session = runtime().session(EventStreamHost::new(capabilities())).unwrap();
        session.eval("panel.new(2) { title = \"hello\" }").unwrap();
        let crate::HostEvent::Emit { handle, tree } = &session.host().events[0] else { panic!() };
        assert!(session.context().owns_component(*handle));
        assert_eq!(tree.component_type, "Panel");
    }

    #[test]
    fn emitted_component_identity_comes_from_host_response() {
        let handle = crate::ComponentHandle::from_raw(0xfeed_face);
        let host = FixedHandleHost { capabilities: capabilities(), handle, requests: vec![] };
        let mut session = runtime().session(host).unwrap();
        let result = session.eval("panel.new(2) { title = \"hello\" }").unwrap();
        assert_eq!(result.value, Some(Value::ComponentObject { id: handle, component_type: "Panel".into() }));
        assert!(session.context().owns_component(handle));
        assert!(matches!(session.host().requests[0], HostRequest::Emit { .. }));
    }

    #[test]
    fn materializes_component_without_host_session() {
        let tree = runtime().materialize_component("panel.new(2) { title = \"hello\" }").unwrap();
        assert_eq!(tree.component_type, "Panel");
        assert_eq!(tree.ctor_method, Some("new".into()));
        assert_eq!(tree.ctor_args, vec![Value::Number(2.0)]);
        assert_eq!(tree.named, vec![("title".into(), Value::String("hello".into()))]);
    }

    #[test]
    fn bindings_and_table_identity_persist_between_evaluations() {
        let mut session = runtime().session(EventStreamHost::new(capabilities())).unwrap();
        session.eval("let table = { value = 1 }; let alias = table").unwrap();
        session.eval("alias[\"value\"] = 9").unwrap();
        let result = session.eval("table[\"value\"]").unwrap();
        assert_eq!(result.value, Some(Value::Number(9.0)));
    }

    #[test]
    fn namespace_api_is_transport_safe() {
        let mut session = runtime().session(EventStreamHost::new(capabilities())).unwrap();
        session.eval("log.write(\"hello\")").unwrap();
        assert!(matches!(&session.host().events[0], crate::HostEvent::Api { id, args }
            if id == "log.write" && args == &vec![TransportValue::String("hello".into())]));
    }

    #[test]
    fn duplicate_and_capability_failures_are_typed() {
        let mut builder = Runtime::builder();
        builder.register_component(ComponentSpec::new("Panel")).unwrap();
        assert_eq!(builder.register_component(ComponentSpec::new("panel")).unwrap_err().kind, CatalogErrorKind::DuplicateName);
        let error = match builder.build().session(EventStreamHost::new(HostCapabilities::default())) {
            Err(error) => error,
            Ok(_) => panic!("expected capability mismatch"),
        };
        assert_eq!(error.kind, CatalogErrorKind::CapabilityMismatch);
    }

    #[test]
    fn suggestions_and_schema_validation_are_reported() {
        let mut session = runtime().session(EventStreamHost::new(capabilities())).unwrap();
        let error = session.eval("panel.neew(2)").unwrap_err().to_string();
        assert!(error.contains("did you mean 'new'"), "{error}");
        let error = session.eval("panel.new(\"bad\")").unwrap_err().to_string();
        assert!(error.contains("wrong type"), "{error}");
    }

    #[test]
    fn host_boundary_rejects_cyclic_tables() {
        let mut session = runtime().session(EventStreamHost::new(capabilities())).unwrap();
        session.eval("let table = { label = \"root\" }; table[\"self\"] = table").unwrap();
        let error = session.eval("sink.write(table)").unwrap_err().to_string();
        assert!(error.contains("cyclic table"), "{error}");
    }

    #[test]
    fn dotted_unknown_component_suggests_registered_names() {
        let mut session = runtime().session(EventStreamHost::new(capabilities())).unwrap();
        let error = session.eval("panal.new(2)").unwrap_err().to_string();
        assert!(error.contains("did you mean 'panel'"), "{error}");
    }
}
