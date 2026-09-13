//! Configurable MMS runtime and persistent sessions.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
    CallbackHandle, EvalError, Evaluation, Evaluator, Expression, HeapHandle, Host, HostContext,
    Hostless, MaterializedCE, MeowMeowParser, MeowMeowTokenizer, Statement, Value,
};

static NEXT_SESSION_TAG: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentNamePolicy {
    OpenUppercase,
    StrictRegistered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Any,
    Null,
    Bool,
    /// Compatibility type for the pre-0.8 single-`f64` numeric surface.
    /// New RuntimeSpecs should use a fixed-width numeric type.
    Number,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Array,
    Table,
    Component,
    Callback,
}

impl ValueType {
    pub fn accepts(&self, value: &Value) -> bool {
        matches!(self, Self::Any)
            || matches!(
                (self, value),
                (Self::Null, Value::Null)
                    | (Self::Bool, Value::Bool(_))
                    | (Self::Number, Value::Number(_))
                    | (Self::String, Value::String(_))
                    | (Self::Array, Value::Array(_))
                    | (Self::Table, Value::Map(_) | Value::Object(_))
                    | (Self::Component, Value::ComponentObject { .. })
                    | (Self::Callback, Value::Function { .. })
            )
            || match (self, value) {
                (Self::I8, Value::Number(value)) => integer_in_range(*value, i8::MIN, i8::MAX),
                (Self::I16, Value::Number(value)) => integer_in_range(*value, i16::MIN, i16::MAX),
                (Self::I32, Value::Number(value)) => integer_in_range(*value, i32::MIN, i32::MAX),
                (Self::I64, Value::Number(value)) => {
                    value.is_finite()
                        && value.fract() == 0.0
                        && *value >= i64::MIN as f64
                        && *value < 9_223_372_036_854_775_808.0
                }
                (Self::U8, Value::Number(value)) => unsigned_integer_in_range(*value, u8::MAX),
                (Self::U16, Value::Number(value)) => unsigned_integer_in_range(*value, u16::MAX),
                (Self::U32, Value::Number(value)) => unsigned_integer_in_range(*value, u32::MAX),
                (Self::U64, Value::Number(value)) => {
                    value.is_finite()
                        && value.fract() == 0.0
                        && *value >= 0.0
                        && *value < 18_446_744_073_709_551_616.0
                }
                // Until numeric values retain their source width, an ordinary
                // Number is contextually accepted at either floating boundary.
                (Self::F32, Value::Number(value)) => {
                    !value.is_finite() || (*value as f32).is_finite()
                }
                (Self::F64, Value::Number(_)) => true,
                _ => false,
            }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Number => "number",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "str",
            Self::Array => "array",
            Self::Table => "table",
            Self::Component => "component",
            Self::Callback => "callback",
        }
    }
}

fn integer_in_range<T>(value: f64, min: T, max: T) -> bool
where
    T: Copy + Into<i64>,
{
    value.is_finite()
        && value.fract() == 0.0
        && value >= min.into() as f64
        && value <= max.into() as f64
}

fn unsigned_integer_in_range<T>(value: f64, max: T) -> bool
where
    T: Copy + Into<u64>,
{
    value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= max.into() as f64
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueSignature {
    pub arguments: Vec<ValueType>,
    /// The number of leading arguments which must be supplied. Remaining
    /// declared arguments are optional and retain their declared types.
    pub minimum_arguments: usize,
    pub result: ValueType,
    pub variadic: bool,
}

/// Opaque identity assigned to a declaration whose implementation crosses the
/// host boundary. IDs can only be allocated by [`RuntimeSpecBuilder`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId(u32);

impl fmt::Debug for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OperationId").field(&self.0).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentBodyMode {
    /// Evaluate the body immediately as a structural MMS block. Component
    /// values produced by expressions become children in authored order.
    Standard,
    /// Like [`Self::Standard`], but identifier assignments are component
    /// properties even when a lexical binding with the same name exists.
    PropsOnly,
    /// Preserve the body as an executable closure owned by the component.
    /// This is intended for imperative components such as keyframes.
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImplementationTarget {
    Pure,
    Host(OperationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableDeclaration {
    name: String,
    signature: ValueSignature,
    target: ImplementationTarget,
}

impl CallableDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn signature(&self) -> &ValueSignature {
        &self.signature
    }
    pub fn operation_id(&self) -> Option<OperationId> {
        match self.target {
            ImplementationTarget::Pure => None,
            ImplementationTarget::Host(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDeclaration {
    name: String,
    value_type: ValueType,
    target: ImplementationTarget,
}

impl PropertyDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value_type(&self) -> &ValueType {
        &self.value_type
    }
    pub fn operation_id(&self) -> Option<OperationId> {
        match self.target {
            ImplementationTarget::Pure => None,
            ImplementationTarget::Host(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalDeclaration {
    name: String,
    fields: Vec<(String, ValueType)>,
    operation_id: OperationId,
}

impl SignalDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (&str, &ValueType)> {
        self.fields.iter().map(|(name, ty)| (name.as_str(), ty))
    }
    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDeclaration {
    name: String,
    default_constructor: ImplementationTarget,
    aliases: Vec<String>,
    body_mode: ComponentBodyMode,
    constructors: Vec<CallableDeclaration>,
    builder_calls: Vec<CallableDeclaration>,
    positionals: Vec<ValueType>,
    properties: Vec<PropertyDeclaration>,
    methods: Vec<CallableDeclaration>,
}

impl ComponentDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn default_constructor_operation_id(&self) -> Option<OperationId> {
        match self.default_constructor {
            ImplementationTarget::Pure => None,
            ImplementationTarget::Host(id) => Some(id),
        }
    }
    pub fn aliases(&self) -> impl ExactSizeIterator<Item = &str> {
        self.aliases.iter().map(String::as_str)
    }
    pub fn body_mode(&self) -> ComponentBodyMode {
        self.body_mode
    }
    pub fn constructors(&self) -> impl ExactSizeIterator<Item = &CallableDeclaration> {
        self.constructors.iter()
    }
    pub fn builder_calls(&self) -> impl ExactSizeIterator<Item = &CallableDeclaration> {
        self.builder_calls.iter()
    }
    pub fn positionals(&self) -> impl ExactSizeIterator<Item = &ValueType> {
        self.positionals.iter()
    }
    pub fn properties(&self) -> impl ExactSizeIterator<Item = &PropertyDeclaration> {
        self.properties.iter()
    }
    pub fn methods(&self) -> impl ExactSizeIterator<Item = &CallableDeclaration> {
        self.methods.iter()
    }
    pub fn method(&self, name: &str) -> Option<&CallableDeclaration> {
        self.methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinDeclaration {
    name: String,
    signature: ValueSignature,
    target: ImplementationTarget,
}

impl BuiltinDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn signature(&self) -> &ValueSignature {
        &self.signature
    }
    pub fn operation_id(&self) -> Option<OperationId> {
        match self.target {
            ImplementationTarget::Pure => None,
            ImplementationTarget::Host(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiDeclaration {
    namespace: Option<String>,
    name: String,
    signature: ValueSignature,
    operation_id: OperationId,
}

impl ApiDeclaration {
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn signature(&self) -> &ValueSignature {
        &self.signature
    }
    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
}

/// Immutable, crate-owned description of one MMS vocabulary.
#[derive(Debug, Clone)]
pub struct RuntimeSpec {
    component_name_policy: ComponentNamePolicy,
    components: Vec<ComponentDeclaration>,
    builtins: Vec<BuiltinDeclaration>,
    apis: Vec<ApiDeclaration>,
    signals: Vec<SignalDeclaration>,
}

impl RuntimeSpec {
    pub fn builder<I>() -> RuntimeSpecBuilder<I> {
        RuntimeSpecBuilder::new()
    }
    pub fn component_name_policy(&self) -> ComponentNamePolicy {
        self.component_name_policy
    }
    pub fn components(&self) -> impl ExactSizeIterator<Item = &ComponentDeclaration> {
        self.components.iter()
    }
    pub fn builtins(&self) -> impl ExactSizeIterator<Item = &BuiltinDeclaration> {
        self.builtins.iter()
    }
    pub fn apis(&self) -> impl ExactSizeIterator<Item = &ApiDeclaration> {
        self.apis.iter()
    }
    pub fn signals(&self) -> impl ExactSizeIterator<Item = &SignalDeclaration> {
        self.signals.iter()
    }
    pub fn signal(&self, name: &str) -> Option<&SignalDeclaration> {
        self.signals
            .iter()
            .find(|signal| signal.name.eq_ignore_ascii_case(name))
    }
    pub fn component(&self, name: &str) -> Option<&ComponentDeclaration> {
        self.components.iter().find(|component| {
            component.name.eq_ignore_ascii_case(name)
                || component
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(name))
        })
    }
    pub fn api(&self, namespace: Option<&str>, name: &str) -> Option<&ApiDeclaration> {
        self.apis.iter().find(|api| {
            api.name.eq_ignore_ascii_case(name)
                && match (api.namespace.as_deref(), namespace) {
                    (None, None) => true,
                    (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
                    _ => false,
                }
        })
    }
}

/// Metadata-free host implementation table built alongside a [`RuntimeSpec`].
#[derive(Debug)]
pub struct ImplementationBindings<I> {
    entries: Vec<(OperationId, I)>,
}

impl<I> ImplementationBindings<I> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, id: OperationId) -> Option<&I> {
        self.entries
            .iter()
            .find_map(|(candidate, implementation)| (*candidate == id).then_some(implementation))
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (OperationId, &I)> {
        self.entries
            .iter()
            .map(|(id, implementation)| (*id, implementation))
    }
}

/// The inseparable executable runtime and host bindings produced by one
/// [`RuntimeSpecBuilder`] build.
///
/// An [`OperationId`] is meaningful only with the bindings from the same
/// configured runtime, so this type prevents callers from compiling a spec
/// and accidentally pairing it with a different binding table.
#[derive(Debug)]
pub struct ConfiguredRuntime<I> {
    runtime: Runtime,
    bindings: ImplementationBindings<I>,
}

impl<I> ConfiguredRuntime<I> {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    pub fn spec(&self) -> &RuntimeSpec {
        self.runtime.spec()
    }
    pub fn bindings(&self) -> &ImplementationBindings<I> {
        &self.bindings
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSpecErrorKind {
    DuplicateName,
    NameConflict,
    InvalidNesting,
    InvalidSignature,
    ConflictingSignature,
    MissingImplementation,
    OrphanImplementation,
    DuplicateOperationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSpecError {
    pub kind: RuntimeSpecErrorKind,
    pub path: String,
    pub message: String,
}

impl fmt::Display for RuntimeSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for RuntimeSpecError {}

pub struct RuntimeSpecBuilder<I> {
    component_name_policy: ComponentNamePolicy,
    components: Vec<ComponentDeclaration>,
    builtins: Vec<BuiltinDeclaration>,
    apis: Vec<ApiDeclaration>,
    signals: Vec<SignalDeclaration>,
    bindings: Vec<(OperationId, I)>,
    next_operation_id: u32,
}

impl<I> Default for RuntimeSpecBuilder<I> {
    fn default() -> Self {
        Self {
            component_name_policy: ComponentNamePolicy::StrictRegistered,
            components: vec![],
            builtins: vec![],
            apis: vec![],
            signals: vec![],
            bindings: vec![],
            next_operation_id: 1,
        }
    }
}

impl<I> RuntimeSpecBuilder<I> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn component_name_policy(&mut self, policy: ComponentNamePolicy) -> &mut Self {
        self.component_name_policy = policy;
        self
    }
    pub fn with_standard_builtins(&mut self) -> &mut Self {
        for name in [
            "null",
            "range",
            "len",
            "query",
            "query_all",
            "on",
            "on_global",
            "Math",
            "MusicNote",
        ] {
            self.pure_builtin(name, ValueSignature::any());
        }
        self
    }
    pub fn pure_builtin(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
    ) -> &mut Self {
        self.builtins.push(BuiltinDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Pure,
        });
        self
    }
    pub fn host_builtin(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.builtins.push(BuiltinDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Host(id),
        });
        self
    }
    pub fn component(
        &mut self,
        name: impl Into<String>,
        configure: impl FnOnce(&mut ComponentBuilder<'_, I>),
    ) -> &mut Self {
        let declaration = ComponentDeclaration {
            name: name.into(),
            default_constructor: ImplementationTarget::Pure,
            aliases: vec![],
            body_mode: ComponentBodyMode::Standard,
            constructors: vec![],
            builder_calls: vec![],
            positionals: vec![],
            properties: vec![],
            methods: vec![],
        };
        let mut builder = ComponentBuilder {
            declaration,
            bindings: &mut self.bindings,
            next_operation_id: &mut self.next_operation_id,
        };
        configure(&mut builder);
        self.components.push(builder.declaration);
        self
    }
    /// Declares a component whose default constructor is implemented by the host.
    pub fn host_component(
        &mut self,
        name: impl Into<String>,
        implementation: I,
        configure: impl FnOnce(&mut ComponentBuilder<'_, I>),
    ) -> &mut Self {
        let operation_id = self.bind(implementation);
        let declaration = ComponentDeclaration {
            name: name.into(),
            default_constructor: ImplementationTarget::Host(operation_id),
            aliases: vec![],
            body_mode: ComponentBodyMode::Standard,
            constructors: vec![],
            builder_calls: vec![],
            positionals: vec![],
            properties: vec![],
            methods: vec![],
        };
        let mut builder = ComponentBuilder {
            declaration,
            bindings: &mut self.bindings,
            next_operation_id: &mut self.next_operation_id,
        };
        configure(&mut builder);
        self.components.push(builder.declaration);
        self
    }
    pub fn host_api(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.apis.push(ApiDeclaration {
            namespace: None,
            name: name.into(),
            signature,
            operation_id: id,
        });
        self
    }
    /// Declare a host event kind. Signal declarations are global vocabulary;
    /// registration may optionally supply a component scope.
    pub fn signal(
        &mut self,
        name: impl Into<String>,
        fields: impl Into<Vec<(String, ValueType)>>,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.signals.push(SignalDeclaration {
            name: name.into(),
            fields: fields.into(),
            operation_id: id,
        });
        self
    }
    pub fn namespace(
        &mut self,
        name: impl Into<String>,
        configure: impl FnOnce(&mut NamespaceBuilder<'_, I>),
    ) -> &mut Self {
        let mut builder = NamespaceBuilder {
            name: name.into(),
            apis: &mut self.apis,
            bindings: &mut self.bindings,
            next_operation_id: &mut self.next_operation_id,
        };
        configure(&mut builder);
        self
    }
    pub fn build(self) -> Result<ConfiguredRuntime<I>, RuntimeSpecError> {
        validate_runtime_spec(&self.components, &self.builtins, &self.apis, &self.signals)?;
        validate_bindings(
            &self.components,
            &self.builtins,
            &self.apis,
            &self.signals,
            &self.bindings,
        )?;
        let spec = RuntimeSpec {
            component_name_policy: self.component_name_policy,
            components: self.components,
            builtins: self.builtins,
            apis: self.apis,
            signals: self.signals,
        };
        Ok(ConfiguredRuntime {
            runtime: Runtime::from_spec(spec),
            bindings: ImplementationBindings {
                entries: self.bindings,
            },
        })
    }
    fn bind(&mut self, implementation: I) -> OperationId {
        let id = OperationId(self.next_operation_id);
        self.next_operation_id = self
            .next_operation_id
            .checked_add(1)
            .expect("runtime operation ID space exhausted");
        self.bindings.push((id, implementation));
        id
    }
}

pub struct ComponentBuilder<'a, I> {
    declaration: ComponentDeclaration,
    bindings: &'a mut Vec<(OperationId, I)>,
    next_operation_id: &'a mut u32,
}

impl<I> ComponentBuilder<'_, I> {
    pub fn alias(&mut self, alias: impl Into<String>) -> &mut Self {
        self.declaration.aliases.push(alias.into());
        self
    }
    /// Marks this component's default constructor as host-implemented.
    pub fn host_default_constructor(&mut self, implementation: I) -> &mut Self {
        let id = self.bind(implementation);
        self.declaration.default_constructor = ImplementationTarget::Host(id);
        self
    }
    pub fn body_mode(&mut self, mode: ComponentBodyMode) -> &mut Self {
        self.declaration.body_mode = mode;
        self
    }
    pub fn positional(&mut self, ty: ValueType) -> &mut Self {
        self.declaration.positionals.push(ty);
        self
    }
    pub fn constructor(&mut self, name: impl Into<String>, signature: ValueSignature) -> &mut Self {
        self.declaration.constructors.push(CallableDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Pure,
        });
        self
    }
    pub fn host_constructor(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.declaration.constructors.push(CallableDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Host(id),
        });
        self
    }
    pub fn builder_call(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
    ) -> &mut Self {
        self.declaration.builder_calls.push(CallableDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Pure,
        });
        self
    }
    pub fn host_builder_call(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.declaration.builder_calls.push(CallableDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Host(id),
        });
        self
    }
    pub fn property(&mut self, name: impl Into<String>, ty: ValueType) -> &mut Self {
        self.declaration.properties.push(PropertyDeclaration {
            name: name.into(),
            value_type: ty,
            target: ImplementationTarget::Pure,
        });
        self
    }
    pub fn host_property(
        &mut self,
        name: impl Into<String>,
        ty: ValueType,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.declaration.properties.push(PropertyDeclaration {
            name: name.into(),
            value_type: ty,
            target: ImplementationTarget::Host(id),
        });
        self
    }
    pub fn method(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
        implementation: I,
    ) -> &mut Self {
        let id = self.bind(implementation);
        self.declaration.methods.push(CallableDeclaration {
            name: name.into(),
            signature,
            target: ImplementationTarget::Host(id),
        });
        self
    }
    fn bind(&mut self, implementation: I) -> OperationId {
        let id = OperationId(*self.next_operation_id);
        *self.next_operation_id = self
            .next_operation_id
            .checked_add(1)
            .expect("runtime operation ID space exhausted");
        self.bindings.push((id, implementation));
        id
    }
}

pub struct NamespaceBuilder<'a, I> {
    name: String,
    apis: &'a mut Vec<ApiDeclaration>,
    bindings: &'a mut Vec<(OperationId, I)>,
    next_operation_id: &'a mut u32,
}

impl<I> NamespaceBuilder<'_, I> {
    pub fn api(
        &mut self,
        name: impl Into<String>,
        signature: ValueSignature,
        implementation: I,
    ) -> &mut Self {
        let id = OperationId(*self.next_operation_id);
        *self.next_operation_id = self
            .next_operation_id
            .checked_add(1)
            .expect("runtime operation ID space exhausted");
        self.bindings.push((id, implementation));
        self.apis.push(ApiDeclaration {
            namespace: Some(self.name.clone()),
            name: name.into(),
            signature,
            operation_id: id,
        });
        self
    }
}

fn validate_runtime_spec(
    components: &[ComponentDeclaration],
    builtins: &[BuiltinDeclaration],
    apis: &[ApiDeclaration],
    signals: &[SignalDeclaration],
) -> Result<(), RuntimeSpecError> {
    let mut global_names = HashMap::<String, String>::new();
    for builtin in builtins {
        validate_declaration_name(&builtin.name, &builtin.name)?;
        validate_signature(&builtin.name, &builtin.signature)?;
        claim_name(&mut global_names, &builtin.name, &builtin.name)?;
    }
    for component in components {
        validate_declaration_name(&component.name, &component.name)?;
        claim_name(&mut global_names, &component.name, &component.name)?;
        for alias in &component.aliases {
            let path = format!("{}.alias({alias})", component.name);
            validate_declaration_name(alias, &path)?;
            claim_name(&mut global_names, alias, &path)?;
        }
        validate_named(
            &component.name,
            "constructor",
            component
                .constructors
                .iter()
                .map(|item| (item.name.clone(), item.signature.clone())),
        )?;
        validate_named(
            &component.name,
            "builder_call",
            component
                .builder_calls
                .iter()
                .map(|item| (item.name.clone(), item.signature.clone())),
        )?;
        validate_named(
            &component.name,
            "property",
            component.properties.iter().map(|item| {
                (
                    item.name.clone(),
                    ValueSignature::new(vec![], item.value_type.clone()),
                )
            }),
        )?;
        validate_named(
            &component.name,
            "method",
            component
                .methods
                .iter()
                .map(|item| (item.name.clone(), item.signature.clone())),
        )?;
    }
    validate_named(
        "RuntimeSpec",
        "signal",
        signals.iter().map(|item| {
            (
                item.name.clone(),
                ValueSignature::new(
                    item.fields
                        .iter()
                        .map(|(_, ty)| ty.clone())
                        .collect::<Vec<_>>(),
                    ValueType::Null,
                ),
            )
        }),
    )?;
    for signal in signals {
        let mut fields = HashSet::new();
        for (field, _) in &signal.fields {
            validate_declaration_name(field, &format!("signal({}).field({field})", signal.name))?;
            if !fields.insert(field.to_lowercase()) {
                return Err(spec_error(
                    RuntimeSpecErrorKind::DuplicateName,
                    format!("signal({}).field({field})", signal.name),
                    format!("duplicate signal field '{field}'"),
                ));
            }
        }
    }
    let mut namespaces = HashMap::<String, String>::new();
    let component_names: HashSet<_> = components
        .iter()
        .map(|component| component.name.to_lowercase())
        .collect();
    for api in apis {
        let path = api_path(api.namespace.as_deref(), &api.name);
        validate_declaration_name(&api.name, &path)?;
        validate_signature(&path, &api.signature)?;
        if let Some(namespace) = &api.namespace {
            validate_declaration_name(namespace, namespace)?;
            if !namespaces.contains_key(&namespace.to_lowercase()) {
                if !component_names.contains(&namespace.to_lowercase()) {
                    claim_name(&mut global_names, namespace, namespace)?;
                }
                namespaces.insert(namespace.to_lowercase(), namespace.clone());
            }
        } else {
            claim_name(&mut global_names, &api.name, &api.name)?;
        }
    }
    let mut api_names = HashSet::new();
    for api in apis {
        let key = api_key(api.namespace.as_deref(), &api.name);
        if !api_names.insert(key) {
            return Err(spec_error(
                RuntimeSpecErrorKind::DuplicateName,
                api_path(api.namespace.as_deref(), &api.name),
                "duplicate API declaration".into(),
            ));
        }
    }
    Ok(())
}

fn validate_declaration_name(name: &str, path: &str) -> Result<(), RuntimeSpecError> {
    if name.trim().is_empty() || name.contains('.') {
        return Err(spec_error(
            RuntimeSpecErrorKind::InvalidNesting,
            path.into(),
            format!("'{name}' is not a valid declaration name"),
        ));
    }
    Ok(())
}

fn validate_named(
    component: &str,
    kind: &str,
    items: impl Iterator<Item = (String, ValueSignature)>,
) -> Result<(), RuntimeSpecError> {
    let mut seen = HashMap::<String, ValueSignature>::new();
    for (name, signature) in items {
        let path = format!("{component}.{kind}({name})");
        validate_declaration_name(&name, &path)?;
        validate_signature(&path, &signature)?;
        let key = name.to_lowercase();
        if let Some(previous) = seen.get(&key) {
            let error_kind = if previous == &signature {
                RuntimeSpecErrorKind::DuplicateName
            } else {
                RuntimeSpecErrorKind::ConflictingSignature
            };
            return Err(spec_error(
                error_kind,
                format!("{component}.{kind}({name})"),
                format!("duplicate {kind} '{name}'"),
            ));
        }
        seen.insert(key, signature);
    }
    Ok(())
}

fn validate_signature(path: &str, signature: &ValueSignature) -> Result<(), RuntimeSpecError> {
    if signature.minimum_arguments > signature.arguments.len() {
        return Err(spec_error(
            RuntimeSpecErrorKind::InvalidSignature,
            path.into(),
            format!(
                "minimum argument count {} exceeds {} declared arguments",
                signature.minimum_arguments,
                signature.arguments.len()
            ),
        ));
    }
    Ok(())
}

fn validate_bindings<I>(
    components: &[ComponentDeclaration],
    builtins: &[BuiltinDeclaration],
    apis: &[ApiDeclaration],
    signals: &[SignalDeclaration],
    bindings: &[(OperationId, I)],
) -> Result<(), RuntimeSpecError> {
    let mut declarations = HashMap::<OperationId, String>::new();
    let mut declare = |id: OperationId, path: String| {
        if declarations.insert(id, path.clone()).is_some() {
            Err(spec_error(
                RuntimeSpecErrorKind::DuplicateOperationId,
                path,
                "operation ID is assigned to more than one declaration".into(),
            ))
        } else {
            Ok(())
        }
    };
    for builtin in builtins {
        if let Some(id) = builtin.operation_id() {
            declare(id, builtin.name.clone())?;
        }
    }
    for component in components {
        if let Some(id) = component.default_constructor_operation_id() {
            declare(id, format!("{}.constructor(default)", component.name))?;
        }
        for item in &component.constructors {
            if let Some(id) = item.operation_id() {
                declare(id, format!("{}.constructor({})", component.name, item.name))?;
            }
        }
        for item in &component.builder_calls {
            if let Some(id) = item.operation_id() {
                declare(
                    id,
                    format!("{}.builder_call({})", component.name, item.name),
                )?;
            }
        }
        for item in &component.properties {
            if let Some(id) = item.operation_id() {
                declare(id, format!("{}.property({})", component.name, item.name))?;
            }
        }
        for item in &component.methods {
            if let Some(id) = item.operation_id() {
                declare(id, format!("{}.method({})", component.name, item.name))?;
            }
        }
    }
    for item in signals {
        declare(item.operation_id, format!("signal({})", item.name))?;
    }
    for api in apis {
        declare(
            api.operation_id,
            api_path(api.namespace.as_deref(), &api.name),
        )?;
    }

    let mut bound = HashSet::new();
    for (id, _) in bindings {
        if !bound.insert(*id) {
            return Err(spec_error(
                RuntimeSpecErrorKind::DuplicateOperationId,
                format!("{id:?}"),
                "operation ID has more than one implementation".into(),
            ));
        }
        if !declarations.contains_key(id) {
            return Err(spec_error(
                RuntimeSpecErrorKind::OrphanImplementation,
                format!("{id:?}"),
                "implementation is not reachable from a declaration".into(),
            ));
        }
    }
    if let Some((_, path)) = declarations.iter().find(|(id, _)| !bound.contains(id)) {
        return Err(spec_error(
            RuntimeSpecErrorKind::MissingImplementation,
            path.clone(),
            "host-effectful declaration has no implementation".into(),
        ));
    }
    Ok(())
}

fn claim_name(
    names: &mut HashMap<String, String>,
    name: &str,
    path: &str,
) -> Result<(), RuntimeSpecError> {
    let key = name.to_lowercase();
    if let Some(previous) = names.get(&key) {
        let kind = if previous == name {
            RuntimeSpecErrorKind::DuplicateName
        } else {
            RuntimeSpecErrorKind::NameConflict
        };
        return Err(spec_error(
            kind,
            path.into(),
            format!("name '{name}' conflicts with '{previous}'"),
        ));
    }
    names.insert(key, name.into());
    Ok(())
}

fn spec_error(kind: RuntimeSpecErrorKind, path: String, message: String) -> RuntimeSpecError {
    RuntimeSpecError {
        kind,
        message: format!("{path}: {message}"),
        path,
    }
}

fn api_path(namespace: Option<&str>, name: &str) -> String {
    namespace.map_or_else(
        || name.into(),
        |namespace| format!("{namespace}.api({name})"),
    )
}

impl ValueSignature {
    pub fn new(arguments: impl Into<Vec<ValueType>>, result: ValueType) -> Self {
        let arguments = arguments.into();
        let minimum_arguments = arguments.len();
        Self {
            arguments,
            minimum_arguments,
            result,
            variadic: false,
        }
    }
    /// Declares a bounded signature whose trailing arguments may be omitted.
    pub fn with_optional(
        arguments: impl Into<Vec<ValueType>>,
        minimum_arguments: usize,
        result: ValueType,
    ) -> Self {
        let arguments = arguments.into();
        Self {
            arguments,
            minimum_arguments,
            result,
            variadic: false,
        }
    }
    pub fn any() -> Self {
        Self {
            arguments: vec![],
            minimum_arguments: 0,
            result: ValueType::Any,
            variadic: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentSpec {
    pub name: String,
    pub aliases: Vec<String>,
    pub body_mode: ComponentBodyMode,
    pub constructors: HashMap<String, ValueSignature>,
    pub builder_calls: HashMap<String, ValueSignature>,
    pub properties: HashMap<String, ValueType>,
    pub positional: Vec<ValueType>,
    pub methods: HashMap<String, ValueSignature>,
}

#[derive(Debug, Clone)]
pub(crate) struct HostApiSpec {
    pub id: String,
    pub signature: ValueSignature,
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentOperationIds {
    pub default_constructor: Option<OperationId>,
    pub constructors: HashMap<String, OperationId>,
    pub builder_calls: HashMap<String, OperationId>,
    pub properties: HashMap<String, OperationId>,
    pub methods: HashMap<String, OperationId>,
}

#[derive(Debug, Clone)]
pub(crate) struct Catalog {
    pub component_name_policy: ComponentNamePolicy,
    pub components: HashMap<String, Arc<ComponentSpec>>,
    pub component_operations: HashMap<String, Arc<ComponentOperationIds>>,
    pub apis: HashMap<String, Arc<HostApiSpec>>,
    pub api_operation_ids: HashMap<String, OperationId>,
    pub signals: HashMap<String, SignalDeclaration>,
    pub namespaces: HashSet<String>,
    pub builtins: HashSet<String>,
}

impl Catalog {
    pub(crate) fn has_namespace(&self, name: &str) -> bool {
        self.namespaces
            .iter()
            .any(|namespace| namespace.eq_ignore_ascii_case(name))
    }

    pub(crate) fn parser_namespaces(&self) -> impl Iterator<Item = String> + '_ {
        self.namespaces
            .iter()
            .filter(|namespace| !self.components.contains_key(&namespace.to_lowercase()))
            .cloned()
    }

    pub(crate) fn static_component_api_paths(&self) -> impl Iterator<Item = String> + '_ {
        self.apis.keys().filter_map(|path| {
            let (namespace, _) = path.split_once('.')?;
            self.components
                .contains_key(&namespace.to_lowercase())
                .then(|| path.clone())
        })
    }
}

pub(crate) fn api_key(namespace: Option<&str>, name: &str) -> String {
    namespace.map_or_else(
        || name.to_lowercase(),
        |ns| format!("{}.{}", ns.to_lowercase(), name.to_lowercase()),
    )
}

#[derive(Debug, Clone)]
pub struct Runtime {
    spec: Arc<RuntimeSpec>,
    pub(crate) catalog: Arc<Catalog>,
}
impl Runtime {
    pub fn from_spec(spec: RuntimeSpec) -> Self {
        let catalog = compile_catalog(&spec);
        Self {
            spec: Arc::new(spec),
            catalog: Arc::new(catalog),
        }
    }
    pub fn standard() -> Self {
        let mut builder = RuntimeSpec::builder::<()>();
        builder
            .component_name_policy(ComponentNamePolicy::OpenUppercase)
            .with_standard_builtins();
        let build = builder
            .build()
            .expect("the standard runtime specification is valid");
        build.runtime
    }
    pub fn spec(&self) -> &RuntimeSpec {
        &self.spec
    }
    pub fn component_names(&self) -> impl Iterator<Item = &str> {
        self.catalog.components.keys().map(String::as_str)
    }
    pub fn materialize_component(&self, source: &str) -> Result<MaterializedCE, EvalError> {
        let tokens = MeowMeowTokenizer::new(source)
            .tokenize()
            .map_err(|e| EvalError::Tokenize(format!("{e:?}")))?;
        let parser = match self.catalog.component_name_policy {
            ComponentNamePolicy::OpenUppercase => {
                MeowMeowParser::with_open_component_names_namespaces_and_static_apis(
                    tokens,
                    self.catalog.components.keys().cloned(),
                    self.catalog.parser_namespaces(),
                    self.catalog.static_component_api_paths(),
                )
            }
            ComponentNamePolicy::StrictRegistered => {
                MeowMeowParser::with_component_names_namespaces_and_static_apis(
                    tokens,
                    self.catalog.components.keys().cloned(),
                    self.catalog.parser_namespaces(),
                    self.catalog.static_component_api_paths(),
                )
            }
        };
        let statements = parser
            .parse_program()
            .map_err(|e| EvalError::Parse(e.message))?;
        let [Statement::Expression(Expression::Component(component))] = statements.as_slice()
        else {
            return Err(EvalError::Runtime(
                "expected exactly one component expression".into(),
            ));
        };
        let mut host = Hostless;
        let mut evaluator = Evaluator::for_materialization(&mut host, self.catalog.clone());
        evaluator.materialize(component)
    }
    pub fn session<H: Host>(&self, host: H) -> Session<H> {
        let tag = NEXT_SESSION_TAG.fetch_add(1, Ordering::Relaxed);
        Session {
            runtime: self.clone(),
            host,
            scopes: vec![HashMap::from([("null".into(), Value::Null)])],
            heap: HeapHandle::new(),
            callbacks: HashMap::new(),
            module_cache: HashMap::new(),
            context: HostContext::new(tag),
        }
    }
}

fn compile_catalog(spec: &RuntimeSpec) -> Catalog {
    let mut catalog = Catalog {
        component_name_policy: spec.component_name_policy,
        components: HashMap::new(),
        apis: HashMap::new(),
        namespaces: HashSet::new(),
        component_operations: HashMap::new(),
        api_operation_ids: HashMap::new(),
        signals: spec
            .signals
            .iter()
            .map(|signal| (signal.name.to_lowercase(), signal.clone()))
            .collect(),
        builtins: spec
            .builtins
            .iter()
            .map(|builtin| builtin.name.clone())
            .collect(),
    };
    for declaration in &spec.components {
        let operations = Arc::new(ComponentOperationIds {
            default_constructor: declaration.default_constructor_operation_id(),
            constructors: declaration
                .constructors
                .iter()
                .filter_map(|item| item.operation_id().map(|id| (item.name.to_lowercase(), id)))
                .collect(),
            builder_calls: declaration
                .builder_calls
                .iter()
                .filter_map(|item| item.operation_id().map(|id| (item.name.to_lowercase(), id)))
                .collect(),
            properties: declaration
                .properties
                .iter()
                .filter_map(|item| item.operation_id().map(|id| (item.name.to_lowercase(), id)))
                .collect(),
            methods: declaration
                .methods
                .iter()
                .filter_map(|item| item.operation_id().map(|id| (item.name.to_lowercase(), id)))
                .collect(),
        });
        let component = Arc::new(ComponentSpec {
            name: declaration.name.clone(),
            aliases: declaration.aliases.clone(),
            body_mode: declaration.body_mode,
            constructors: declaration
                .constructors
                .iter()
                .map(|item| (item.name.clone(), item.signature.clone()))
                .collect(),
            builder_calls: declaration
                .builder_calls
                .iter()
                .map(|item| (item.name.clone(), item.signature.clone()))
                .collect(),
            properties: declaration
                .properties
                .iter()
                .map(|item| (item.name.clone(), item.value_type.clone()))
                .collect(),
            positional: declaration.positionals.clone(),
            methods: declaration
                .methods
                .iter()
                .map(|item| (item.name.clone(), item.signature.clone()))
                .collect(),
        });
        for name in std::iter::once(&component.name).chain(&component.aliases) {
            catalog
                .components
                .insert(name.to_lowercase(), component.clone());
            catalog
                .component_operations
                .insert(name.to_lowercase(), operations.clone());
        }
    }
    for declaration in &spec.apis {
        if let Some(namespace) = &declaration.namespace {
            catalog.namespaces.insert(namespace.clone());
        }
        let id = api_key(declaration.namespace.as_deref(), &declaration.name);
        catalog
            .api_operation_ids
            .insert(id.clone(), declaration.operation_id);
        catalog.apis.insert(
            id.clone(),
            Arc::new(HostApiSpec {
                id,
                signature: declaration.signature.clone(),
            }),
        );
    }
    catalog
}

pub struct Session<H: Host> {
    runtime: Runtime,
    host: H,
    pub(crate) scopes: Vec<HashMap<String, Value>>,
    pub(crate) heap: HeapHandle,
    pub(crate) callbacks: HashMap<CallbackHandle, Value>,
    module_cache: HashMap<crate::SourceId, Value>,
    pub(crate) context: HostContext,
}

impl<H: Host> Session<H> {
    pub fn handle(&self) -> crate::SessionHandle {
        self.context.session_handle()
    }

    /// Stop retaining a callback. Existing host references become stale.
    pub fn release_callback(&mut self, handle: CallbackHandle) -> bool {
        self.callbacks.remove(&handle).is_some()
    }
    /// Temporarily service this session through another host.
    ///
    /// The active session is scoped to `service` and cannot escape it. Once
    /// the closure returns, the original host is restored while lexical
    /// state, callbacks, heap identity, and handle ownership remain attached
    /// to this session and its original runtime.
    pub fn with_host<N: Host, R>(
        self,
        host: N,
        service: impl FnOnce(&mut Session<N>) -> R,
    ) -> (Self, R) {
        let Session {
            runtime,
            host: parked_host,
            scopes,
            heap,
            callbacks,
            module_cache,
            context,
        } = self;
        let mut active = Session {
            runtime,
            host,
            scopes,
            heap,
            callbacks,
            module_cache,
            context,
        };
        let result = service(&mut active);
        let Session {
            runtime,
            host: _,
            scopes,
            heap,
            callbacks,
            module_cache,
            context,
        } = active;
        (
            Session {
                runtime,
                host: parked_host,
                scopes,
                heap,
                callbacks,
                module_cache,
                context,
            },
            result,
        )
    }

    pub fn eval(&mut self, source: &str) -> Result<Evaluation, EvalError> {
        self.eval_with_source_id(source, None)
    }

    pub fn eval_with_source_id(
        &mut self,
        source: &str,
        source_id: Option<crate::SourceId>,
    ) -> Result<Evaluation, EvalError> {
        let scopes = std::mem::take(&mut self.scopes);
        let callbacks = std::mem::take(&mut self.callbacks);
        let module_cache = std::mem::take(&mut self.module_cache);
        let mut evaluator = Evaluator::for_session(
            &mut self.host,
            scopes,
            self.heap.clone(),
            callbacks,
            self.runtime.catalog.clone(),
            &mut self.context,
            source_id,
            module_cache,
        );
        let result = evaluator.evaluate(source);
        let (scopes, callbacks, module_cache) = evaluator.into_session_state();
        self.scopes = scopes;
        self.callbacks = callbacks;
        self.module_cache = module_cache;
        result
    }
    pub fn invoke_callback(
        &mut self,
        handle: CallbackHandle,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        if !self.context.owns_callback(handle) {
            return Err(crate::HostError {
                kind: crate::HostErrorKind::ForeignHandle,
                operation: "invoke_callback".into(),
                message: format!("callback handle {handle:?} belongs to another session"),
            }
            .into());
        }
        let callback = self
            .callbacks
            .get(&handle)
            .cloned()
            .ok_or_else(|| crate::HostError {
                kind: crate::HostErrorKind::StaleHandle,
                operation: "invoke_callback".into(),
                message: format!(
                    "callback handle {handle:?} is no longer retained by this session"
                ),
            })?;
        let scopes = std::mem::take(&mut self.scopes);
        let callbacks = std::mem::take(&mut self.callbacks);
        let module_cache = std::mem::take(&mut self.module_cache);
        let mut evaluator = Evaluator::for_session(
            &mut self.host,
            scopes,
            self.heap.clone(),
            callbacks,
            self.runtime.catalog.clone(),
            &mut self.context,
            None,
            module_cache,
        );
        let result = evaluator.invoke_value(callback, args);
        let (scopes, callbacks, module_cache) = evaluator.into_session_state();
        self.scopes = scopes;
        self.callbacks = callbacks;
        self.module_cache = module_cache;
        result
    }
    pub fn invoke_callback_invocation(
        &mut self,
        invocation: crate::CallbackInvocation,
    ) -> Result<Value, EvalError> {
        if !self.context.owns_callback(invocation.callback) {
            return Err(crate::HostError {
                kind: crate::HostErrorKind::ForeignHandle,
                operation: "invoke_callback".into(),
                message: format!(
                    "callback handle {:?} belongs to another session",
                    invocation.callback
                ),
            }
            .into());
        }
        for argument in &invocation.args {
            adopt_transport_components(&mut self.context, argument);
        }
        let args = invocation
            .args
            .into_iter()
            .map(crate::evaluator::transport_to_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.invoke_callback(invocation.callback, args)
    }
    pub fn host(&self) -> &H {
        &self.host
    }
    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }
    pub fn context(&self) -> &HostContext {
        &self.context
    }
}

fn adopt_transport_components(context: &mut HostContext, value: &crate::TransportValue) {
    match value {
        crate::TransportValue::Component(handle) => context.adopt_component(*handle),
        crate::TransportValue::Array(values) => {
            for value in values {
                adopt_transport_components(context, value);
            }
        }
        crate::TransportValue::Table(fields) => {
            for (_, value) in fields {
                adopt_transport_components(context, value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CeChild, EventStreamHost, HostError, HostErrorKind, HostEvent, HostRequest, HostResponse,
        TransportValue,
    };

    struct FixedHandleHost {
        handle: crate::ComponentHandle,
        requests: Vec<HostRequest>,
    }

    impl crate::Host for FixedHandleHost {
        fn dispatch_with_context(
            &mut self,
            _context: &mut HostContext,
            request: HostRequest,
        ) -> Result<HostResponse, HostError> {
            self.requests.push(request.clone());
            match request {
                HostRequest::Query { .. } => Ok(HostResponse::Component {
                    handle: self.handle,
                    component_type: "Panel".into(),
                }),
                HostRequest::Emit { tree } | HostRequest::RegisterComponent { tree } => {
                    Ok(HostResponse::Component {
                        handle: self.handle,
                        component_type: tree.component_type,
                    })
                }
                HostRequest::InvokeComponentMethod { .. }
                | HostRequest::InvokeComponentMethodByName { .. }
                | HostRequest::RegisterSignalHandler { .. }
                | HostRequest::RegisterSignalHandlerByName { .. } => Ok(HostResponse::Unit),
                other => Err(HostError::unsupported(other.operation_name())),
            }
        }
    }

    fn runtime() -> ConfiguredRuntime<()> {
        let mut builder = RuntimeSpec::builder();
        builder.with_standard_builtins();
        builder.host_component("Panel", (), |component| {
            component
                .host_constructor(
                    "new",
                    ValueSignature::new(vec![ValueType::Number], ValueType::Component),
                    (),
                )
                .host_property("title", ValueType::String, ())
                .method("show", ValueSignature::new(vec![], ValueType::Null), ());
        });
        builder.namespace("Panel", |namespace| {
            namespace.api("devices", ValueSignature::new(vec![], ValueType::Array), ());
        });
        builder.namespace("log", |namespace| {
            namespace.api(
                "write",
                ValueSignature::new(vec![ValueType::String], ValueType::Null),
                (),
            );
        });
        builder.namespace("sink", |namespace| {
            namespace.api(
                "write",
                ValueSignature::new(vec![ValueType::Any], ValueType::Null),
                (),
            );
        });
        builder.signal("Click", Vec::<(String, ValueType)>::new(), ());
        builder.build().unwrap()
    }

    #[test]
    fn catalog_parses_lowercase_aliases_and_issues_handles() {
        let runtime = runtime();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        session.eval("panel.new(2) { title = \"hello\" }").unwrap();
        let crate::HostEvent::Emit { handle, tree } = &session.host().events[0] else {
            panic!()
        };
        assert!(session.context().owns_component(*handle));
        assert_eq!(tree.component_type, "Panel");
    }

    #[test]
    fn emitted_component_identity_comes_from_host_response() {
        let handle = crate::ComponentHandle::from_raw(0xfeed_face);
        let host = FixedHandleHost {
            handle,
            requests: vec![],
        };
        let runtime = runtime();
        let mut session = runtime.runtime().session(host);
        let result = session.eval("panel.new(2) { title = \"hello\" }").unwrap();
        assert_eq!(
            result.value,
            Some(Value::ComponentObject {
                id: handle,
                component_type: "Panel".into()
            })
        );
        assert!(session.context().owns_component(handle));
        assert!(matches!(
            session.host().requests[0],
            HostRequest::Emit { .. }
        ));
    }

    #[test]
    fn materializes_component_without_host_session() {
        let runtime = runtime();
        let tree = runtime
            .runtime()
            .materialize_component("panel.new(2) { title = \"hello\" }")
            .unwrap();
        assert_eq!(tree.component_type, "Panel");
        let constructor = &tree.constructor;
        assert_eq!(constructor.name.as_deref(), Some("new"));
        assert!(constructor.operation_id.is_some());
        assert_eq!(constructor.arguments, vec![Value::Number(2.0)]);
        assert_eq!(tree.properties[0].name, "title");
        assert!(tree.properties[0].operation_id.is_some());
        assert_eq!(tree.properties[0].value, Value::String("hello".into()));
    }

    #[test]
    fn structural_component_bodies_materialize_control_flow_children_in_order() {
        let runtime = runtime();
        let tree = runtime
            .runtime()
            .materialize_component(
                r#"
                Panel.new(99) {
                    Panel.new(-1) {}
                    for row in range(2) {
                        for column in range(3) {
                            if column == 1 {
                                Panel.new(row * 10 + column) {}
                            } else {
                                Panel.new(row * 10 + column) {}
                            }
                        }
                    }
                    Panel.new(100) {}
                }
                "#,
            )
            .unwrap();

        let constructor_values = tree
            .children
            .iter()
            .map(|child| match child {
                CeChild::Spawn(child) => child.constructor.arguments[0].clone(),
                CeChild::Attach(_) => panic!("hostless materialization should not attach handles"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            constructor_values,
            [-1.0, 0.0, 1.0, 2.0, 10.0, 11.0, 12.0, 100.0]
                .into_iter()
                .map(Value::Number)
                .collect::<Vec<_>>()
        );
        assert!(tree.deferred_block.is_none());
    }

    #[test]
    fn structural_component_body_while_break_continue_and_reassignment_are_eager() {
        let runtime = runtime();
        let tree = runtime
            .runtime()
            .materialize_component(
                r#"
                Panel.new(0) {
                    let index = 0
                    while index < 6 {
                        index = index + 1
                        if index == 2 { continue }
                        if index == 5 { break }
                        Panel.new(index) {}
                    }
                }
                "#,
            )
            .unwrap();

        let constructor_values = tree
            .children
            .iter()
            .map(|child| match child {
                CeChild::Spawn(child) => child.constructor.arguments[0].clone(),
                CeChild::Attach(_) => panic!("hostless materialization should not attach handles"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            constructor_values,
            [1.0, 3.0, 4.0]
                .into_iter()
                .map(Value::Number)
                .collect::<Vec<_>>()
        );
        assert!(
            tree.properties
                .iter()
                .all(|property| property.name != "index")
        );
    }

    #[test]
    fn hostless_structural_body_splices_let_bound_components_as_fresh_children() {
        let runtime = runtime();
        let tree = runtime
            .runtime()
            .materialize_component(
                r#"
                Panel.new(0) {
                    let child = Panel.new(7) {}
                    child
                }
                "#,
            )
            .unwrap();

        let [CeChild::Spawn(child)] = tree.children.as_slice() else {
            panic!("let-bound hostless component should remain a fresh structural child")
        };
        assert_eq!(child.constructor.arguments, [Value::Number(7.0)]);
    }

    #[test]
    fn deferred_component_body_mode_preserves_the_body_without_executing_it() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder
            .with_standard_builtins()
            .component("Keyframe", |component| {
                component
                    .body_mode(ComponentBodyMode::Deferred)
                    .constructor(
                        "at",
                        ValueSignature::new(vec![ValueType::Number], ValueType::Component),
                    );
            });
        let runtime = builder.build().unwrap();
        let tree = runtime
            .runtime()
            .materialize_component("Keyframe.at(1) { operation_that_must_not_run() }")
            .unwrap();

        assert!(tree.initializer_calls.is_empty());
        let deferred = tree.deferred_block.expect("deferred keyframe body");
        assert_eq!(deferred.body.statements.len(), 1);
        assert!(deferred.analysis.is_some());
    }

    #[test]
    fn retained_deferred_component_body_is_an_opaque_session_callback() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder
            .with_standard_builtins()
            .component("Keyframe", |component| {
                component
                    .body_mode(ComponentBodyMode::Deferred)
                    .host_constructor(
                        "at",
                        ValueSignature::new(vec![ValueType::Number], ValueType::Component),
                        (),
                    );
            });
        let runtime = builder.build().unwrap();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        let session_handle = session.handle();
        session.eval("Keyframe.at(1) { let captured = 7 }").unwrap();

        let HostEvent::Emit { tree, .. } = &session.host().events[0] else {
            panic!("expected emitted Keyframe")
        };
        assert!(tree.deferred_block.is_none());
        let callback = tree
            .deferred_callback
            .expect("retained deferred callback reference");
        assert_eq!(callback.session, session_handle);
        assert!(session.context().owns_callback(callback.callback));
        session
            .invoke_callback(callback.callback, Vec::new())
            .unwrap();
        assert!(session.release_callback(callback.callback));
        let error = session
            .invoke_callback(callback.callback, Vec::new())
            .unwrap_err();
        assert!(matches!(
            error,
            EvalError::Host(HostError {
                kind: HostErrorKind::StaleHandle,
                ..
            })
        ));
    }

    #[test]
    fn bindings_and_table_identity_persist_between_evaluations() {
        let runtime = runtime();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        session
            .eval("let table = { value = 1 }; let alias = table")
            .unwrap();
        session.eval("alias[\"value\"] = 9").unwrap();
        let result = session.eval("table[\"value\"]").unwrap();
        assert_eq!(result.value, Some(Value::Number(9.0)));
    }

    #[test]
    fn namespace_api_is_transport_safe() {
        let runtime = runtime();
        let api_id = runtime
            .spec()
            .api(Some("log"), "write")
            .unwrap()
            .operation_id();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        session.eval("log.write(\"hello\")").unwrap();
        assert!(
            matches!(&session.host().events[0], crate::HostEvent::ApiById { operation_id, args }
            if *operation_id == api_id && args == &vec![TransportValue::String("hello".into())])
        );
    }

    #[test]
    fn component_name_can_also_own_a_static_api_namespace() {
        let runtime = runtime();
        let devices_id = runtime
            .runtime()
            .spec()
            .api(Some("Panel"), "devices")
            .unwrap()
            .operation_id();
        let mut session = runtime.runtime().session(EventStreamHost::new());

        session.eval("Panel.devices()").unwrap();
        assert!(
            matches!(session.host().events.last(), Some(crate::HostEvent::ApiById { operation_id, args }) if *operation_id == devices_id && args.is_empty())
        );

        session.eval("Panel.new(2) {}").unwrap();
        assert!(matches!(
            session.host().events.last(),
            Some(crate::HostEvent::Emit { tree, .. }) if tree.component_type == "Panel"
        ));
    }

    #[test]
    fn duplicate_runtime_spec_names_are_typed() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder
            .component("Panel", |_| {})
            .component("panel", |_| {});
        assert_eq!(
            builder.build().unwrap_err().kind,
            RuntimeSpecErrorKind::NameConflict
        );
    }

    #[test]
    fn suggestions_and_schema_validation_are_reported() {
        let runtime = runtime();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        let error = session.eval("panel.neew(2)").unwrap_err().to_string();
        assert!(error.contains("did you mean 'new'"), "{error}");
        let error = session.eval("panel.new(\"bad\")").unwrap_err().to_string();
        assert!(error.contains("wrong type"), "{error}");
    }

    #[test]
    fn host_boundary_rejects_cyclic_tables() {
        let runtime = runtime();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        session
            .eval("let table = { label = \"root\" }; table[\"self\"] = table")
            .unwrap();
        let error = session.eval("sink.write(table)").unwrap_err().to_string();
        assert!(error.contains("cyclic table"), "{error}");
    }

    #[test]
    fn dotted_unknown_component_suggests_registered_names() {
        let runtime = runtime();
        let mut session = runtime.runtime().session(EventStreamHost::new());
        let error = session.eval("panal.new(2)").unwrap_err().to_string();
        assert!(error.contains("did you mean 'panel'"), "{error}");
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestBinding {
        DefaultConstruct,
        Construct,
        Rounded,
        SetTitle,
        Show,
        Click,
        Log,
        Clock,
    }

    #[test]
    fn nested_runtime_spec_preserves_order_and_binds_every_host_declaration() {
        let mut builder = RuntimeSpec::builder::<TestBinding>();
        builder
            .component_name_policy(ComponentNamePolicy::StrictRegistered)
            .with_standard_builtins()
            .host_builtin(
                "clock",
                ValueSignature::new(vec![], ValueType::Number),
                TestBinding::Clock,
            )
            .host_component("Panel", TestBinding::DefaultConstruct, |component| {
                component
                    .alias("Pane")
                    .body_mode(ComponentBodyMode::PropsOnly)
                    .host_constructor(
                        "new",
                        ValueSignature::new(vec![ValueType::Number], ValueType::Component),
                        TestBinding::Construct,
                    )
                    .host_builder_call(
                        "rounded",
                        ValueSignature::new(vec![ValueType::Number], ValueType::Component),
                        TestBinding::Rounded,
                    )
                    .positional(ValueType::String)
                    .host_property("title", ValueType::String, TestBinding::SetTitle)
                    .property("debug", ValueType::Bool)
                    .method(
                        "show",
                        ValueSignature::new(vec![], ValueType::Null),
                        TestBinding::Show,
                    );
            })
            .signal(
                "click",
                vec![("button".into(), ValueType::Number)],
                TestBinding::Click,
            )
            .namespace("log", |namespace| {
                namespace.api(
                    "write",
                    ValueSignature::new(vec![ValueType::String], ValueType::Null),
                    TestBinding::Log,
                );
            });

        let build = builder.build().unwrap();
        let panel = build.spec().component("pane").unwrap();
        assert_eq!(panel.name(), "Panel");
        assert_eq!(panel.body_mode(), ComponentBodyMode::PropsOnly);
        assert_eq!(
            panel
                .properties()
                .map(PropertyDeclaration::name)
                .collect::<Vec<_>>(),
            vec!["title", "debug"]
        );
        assert_eq!(
            build
                .spec()
                .signal("click")
                .unwrap()
                .fields()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            vec!["button"]
        );

        let ids = [
            build
                .spec()
                .builtins()
                .find(|builtin| builtin.name() == "clock")
                .unwrap()
                .operation_id()
                .unwrap(),
            panel.default_constructor_operation_id().unwrap(),
            panel.constructors().next().unwrap().operation_id().unwrap(),
            panel
                .builder_calls()
                .next()
                .unwrap()
                .operation_id()
                .unwrap(),
            panel.properties().next().unwrap().operation_id().unwrap(),
            panel.method("show").unwrap().operation_id().unwrap(),
            build.spec().signal("click").unwrap().operation_id(),
            build
                .spec()
                .api(Some("log"), "write")
                .unwrap()
                .operation_id(),
        ];
        assert_eq!(build.bindings().len(), ids.len());
        assert_eq!(
            ids.map(|id| *build.bindings().get(id).unwrap()),
            [
                TestBinding::Clock,
                TestBinding::DefaultConstruct,
                TestBinding::Construct,
                TestBinding::Rounded,
                TestBinding::SetTitle,
                TestBinding::Show,
                TestBinding::Click,
                TestBinding::Log
            ]
        );

        let tree = build
            .runtime()
            .materialize_component("Pane.new(2) { title = \"hello\" }")
            .unwrap();
        assert_eq!(tree.component_type, "Panel");
        assert_eq!(
            tree.constructor.operation_id,
            panel.constructors().next().unwrap().operation_id()
        );
        assert_eq!(
            tree.properties[0].operation_id,
            panel.properties().next().unwrap().operation_id()
        );
        assert!(build.runtime().materialize_component("Unknown {}").is_err());
        let tree = build.runtime().materialize_component("Pane {}").unwrap();
        assert_eq!(tree.constructor.name, None);
        assert_eq!(tree.constructor.arguments, Vec::<Value>::new());
        assert_eq!(
            tree.constructor.operation_id,
            panel.default_constructor_operation_id()
        );
        let tree = build
            .runtime()
            .materialize_component("Pane { rounded(3) }")
            .unwrap();
        assert_eq!(tree.initializer_calls[0].name, "rounded");
        assert_eq!(
            tree.initializer_calls[0].operation_id,
            panel.builder_calls().next().unwrap().operation_id()
        );
        assert_eq!(
            tree.initializer_calls[0].arguments,
            vec![Value::Number(3.0)]
        );
        let error = build
            .runtime()
            .materialize_component("Pane { missing(3) }")
            .unwrap_err();
        assert!(error.to_string().contains("unknown builder call 'missing'"));

        let show_id = panel.method("show").unwrap().operation_id().unwrap();
        let host = FixedHandleHost {
            handle: crate::ComponentHandle::from_raw(7),
            requests: Vec::new(),
        };
        let mut session = build.runtime().session(host);
        session.eval("query(\"#panel\").show()").unwrap();
        assert!(matches!(
            session.host().requests.last(),
            Some(HostRequest::InvokeComponentMethod { operation_id, .. })
                if *operation_id == show_id
        ));
    }

    #[test]
    fn nested_runtime_spec_reports_deterministic_paths_for_conflicts() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder.component("Transform", |component| {
            component
                .method(
                    "set_position",
                    ValueSignature::new(vec![ValueType::Number], ValueType::Null),
                    (),
                )
                .method(
                    "SET_POSITION",
                    ValueSignature::new(vec![ValueType::String], ValueType::Null),
                    (),
                );
        });
        let error = builder.build().unwrap_err();
        assert_eq!(error.kind, RuntimeSpecErrorKind::ConflictingSignature);
        assert_eq!(error.path, "Transform.method(SET_POSITION)");

        let mut builder = RuntimeSpec::builder::<()>();
        builder.component("Panel", |component| {
            component.alias("panel");
        });
        let error = builder.build().unwrap_err();
        assert_eq!(error.kind, RuntimeSpecErrorKind::NameConflict);
        assert_eq!(error.path, "Panel.alias(panel)");
    }

    #[test]
    fn runtime_spec_rejects_invalid_signatures_and_signal_fields_at_build_time() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder.host_api(
            "broken",
            ValueSignature::with_optional(vec![ValueType::Number], 2, ValueType::Null),
            (),
        );
        let error = builder.build().unwrap_err();
        assert_eq!(error.kind, RuntimeSpecErrorKind::InvalidSignature);
        assert_eq!(error.path, "broken");

        let mut builder = RuntimeSpec::builder::<()>();
        builder.signal("Click", vec![("bad.field".into(), ValueType::String)], ());
        let error = builder.build().unwrap_err();
        assert_eq!(error.kind, RuntimeSpecErrorKind::InvalidNesting);
        assert_eq!(error.path, "signal(Click).field(bad.field)");
    }

    #[test]
    fn standard_runtime_is_backed_by_an_open_runtime_spec() {
        let runtime = Runtime::standard();
        assert_eq!(
            runtime.spec().component_name_policy(),
            ComponentNamePolicy::OpenUppercase
        );
        assert!(
            runtime
                .spec()
                .builtins()
                .any(|builtin| builtin.name() == "range")
        );
        assert!(
            runtime
                .materialize_component("Unregistered { title = \"ok\" }")
                .is_ok()
        );
    }

    #[test]
    fn fixed_width_numeric_types_validate_the_temporary_number_representation() {
        assert!(ValueType::I64.accepts(&Value::Number(-7.0)));
        assert!(!ValueType::I64.accepts(&Value::Number(1.5)));
        assert!(ValueType::U32.accepts(&Value::Number(u32::MAX as f64)));
        assert!(!ValueType::U32.accepts(&Value::Number(-1.0)));
        assert!(!ValueType::U32.accepts(&Value::Number(u32::MAX as f64 + 1.0)));
        assert!(!ValueType::U32.accepts(&Value::Number(f64::NAN)));
        assert!(ValueType::F32.accepts(&Value::Number(1.25)));
        assert!(!ValueType::F32.accepts(&Value::Number(f64::MAX)));
        assert!(ValueType::F64.accepts(&Value::Number(f64::MAX)));
        assert_eq!(ValueType::U32.name(), "u32");
    }

    #[test]
    fn optional_signature_arguments_are_bounded_and_typed() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder.component("Mesh", |component| {
            component.constructor(
                "star",
                ValueSignature::with_optional(
                    vec![ValueType::U32, ValueType::F32],
                    0,
                    ValueType::Component,
                ),
            );
        });
        let runtime = builder.build().unwrap();

        assert!(
            runtime
                .runtime()
                .materialize_component("Mesh.star() {}")
                .is_ok()
        );
        assert!(
            runtime
                .runtime()
                .materialize_component("Mesh.star(5) {}")
                .is_ok()
        );
        assert!(
            runtime
                .runtime()
                .materialize_component("Mesh.star(5, 0.4) {}")
                .is_ok()
        );
        assert!(
            runtime
                .runtime()
                .materialize_component("Mesh.star(-1) {}")
                .is_err()
        );
        assert!(
            runtime
                .runtime()
                .materialize_component("Mesh.star(5, \"wide\") {}")
                .is_err()
        );
        assert!(
            runtime
                .runtime()
                .materialize_component("Mesh.star(5, 0.4, 2) {}")
                .is_err()
        );
    }

    #[test]
    fn configured_signal_registration_sends_only_an_operation_id_and_callback_handle() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Binding {
            Construct,
            Click,
        }

        let mut builder = RuntimeSpec::builder::<Binding>();
        builder
            .with_standard_builtins()
            .host_component("Panel", Binding::Construct, |_| {})
            .signal("Click", Vec::new(), Binding::Click);
        let configured = builder.build().unwrap();
        let click_id = configured.spec().signal("Click").unwrap().operation_id();
        let host = FixedHandleHost {
            handle: crate::ComponentHandle::from_raw(7),
            requests: Vec::new(),
        };
        let mut session = configured.runtime().session(host);

        session
            .eval("let button = Panel {}; on(button, \"Click\", fn(event) { return event })")
            .unwrap();

        let HostRequest::RegisterSignalHandler {
            operation_id,
            callback,
            scope: Some(_),
            ..
        } = session.host().requests.last().unwrap()
        else {
            panic!("expected configured signal registration")
        };
        assert_eq!(*operation_id, click_id);
        let callback = *callback;
        assert!(session.context().owns_callback(callback));
        assert_eq!(
            session
                .invoke_callback_invocation(crate::CallbackInvocation {
                    callback,
                    args: vec![TransportValue::Number(7.0)],
                })
                .unwrap(),
            Value::Number(7.0)
        );

        session
            .eval("on_global(\"Click\", \"global-click\", fn(event) {})")
            .unwrap();
        assert!(matches!(
            session.host().requests.last(),
            Some(HostRequest::RegisterSignalHandler { operation_id, scope: None, name: Some(name), .. })
                if *operation_id == click_id && name == "global-click"
        ));
    }

    #[test]
    fn a_scoped_host_lease_preserves_callback_state() {
        let host = FixedHandleHost {
            handle: crate::ComponentHandle::from_raw(42),
            requests: vec![],
        };
        let runtime = runtime();
        let mut session = runtime.runtime().session(host);
        session
            .eval(
                "let state = { value = 0 }; let panel = Panel {}; on(panel, \"Click\", fn(event) { state.value = state.value + 1; return state.value })",
            )
            .unwrap();
        let callback = *session.callbacks.keys().next().unwrap();

        let (session, values) = session.with_host(crate::Hostless, |session| {
            [
                session
                    .invoke_callback(callback, vec![Value::Null])
                    .unwrap(),
                session
                    .invoke_callback(callback, vec![Value::Null])
                    .unwrap(),
            ]
        });
        assert_eq!(values, [Value::Number(1.0), Value::Number(2.0)]);
        assert!(session.context().owns_callback(callback));
    }

    #[test]
    fn callback_invocation_distinguishes_foreign_and_stale_handles() {
        let make_session = || {
            runtime().runtime().session(FixedHandleHost {
                handle: crate::ComponentHandle::from_raw(42),
                requests: vec![],
            })
        };
        let mut owner = make_session();
        owner
            .eval("let panel = Panel {}; on(panel, \"Click\", fn(event) {})")
            .unwrap();
        let callback = *owner.callbacks.keys().next().unwrap();

        let mut other = make_session();
        let EvalError::Host(error) = other.invoke_callback(callback, vec![]).unwrap_err() else {
            panic!("expected a typed foreign callback error")
        };
        assert_eq!(error.kind, crate::HostErrorKind::ForeignHandle);

        owner.callbacks.remove(&callback);
        let EvalError::Host(error) = owner.invoke_callback(callback, vec![]).unwrap_err() else {
            panic!("expected a typed stale callback error")
        };
        assert_eq!(error.kind, crate::HostErrorKind::StaleHandle);
    }

    #[test]
    fn imports_are_resolved_by_the_host_against_the_importer_identity() {
        #[derive(Default)]
        struct SourceHost {
            requests: Vec<(Option<crate::SourceId>, String)>,
        }

        impl crate::Host for SourceHost {
            fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
                let HostRequest::LoadSource {
                    importer,
                    specifier,
                } = request
                else {
                    return Err(HostError::unsupported(request.operation_name()));
                };
                self.requests.push((importer.clone(), specifier.clone()));
                match specifier.as_str() {
                    "child.mms" => Ok(HostResponse::Source(crate::LoadedSource {
                        identity: crate::SourceId::new("/virtual/child.mms"),
                        source: "import { answer } from \"nested.mms\"; export let value = answer"
                            .into(),
                    })),
                    "nested.mms" => Ok(HostResponse::Source(crate::LoadedSource {
                        identity: crate::SourceId::new("/virtual/nested.mms"),
                        source: "export let answer = 7".into(),
                    })),
                    _ => Err(HostError::failure("load_source", "unknown fixture")),
                }
            }
        }

        let mut session = Runtime::standard().session(SourceHost::default());
        let evaluation = session
            .eval_with_source_id(
                "import { value } from \"child.mms\"; value",
                Some(crate::SourceId::new("/virtual/root.mms")),
            )
            .unwrap();
        assert_eq!(evaluation.value, Some(Value::Number(7.0)));
        assert_eq!(
            session.host().requests,
            vec![
                (
                    Some(crate::SourceId::new("/virtual/root.mms")),
                    "child.mms".into(),
                ),
                (
                    Some(crate::SourceId::new("/virtual/child.mms")),
                    "nested.mms".into(),
                ),
            ]
        );
    }

    #[test]
    fn ordinary_and_ast_imports_keep_separate_component_views() {
        #[derive(Default)]
        struct SourceHost {
            next_handle: u64,
            registers: usize,
            method_calls: usize,
        }

        impl crate::Host for SourceHost {
            fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
                match request {
                    HostRequest::LoadSource { .. } => {
                        Ok(HostResponse::Source(crate::LoadedSource {
                            identity: crate::SourceId::new("/virtual/components.mms"),
                            source: r#"
                            export let marker = Panel {}
                            export fn factory() { return Panel {} }
                            export fn update_marker() { marker.show() }
                        "#
                            .into(),
                        }))
                    }
                    HostRequest::RegisterComponent { tree } => {
                        self.next_handle += 1;
                        self.registers += 1;
                        Ok(HostResponse::Component {
                            handle: crate::ComponentHandle::from_raw(self.next_handle),
                            component_type: tree.component_type,
                        })
                    }
                    HostRequest::InvokeComponentMethod { .. }
                    | HostRequest::InvokeComponentMethodByName { .. } => {
                        self.method_calls += 1;
                        Ok(HostResponse::Unit)
                    }
                    HostRequest::Attach { .. } => Ok(HostResponse::Unit),
                    other => Err(HostError::unsupported(other.operation_name())),
                }
            }
        }

        let mut session = runtime().runtime().session(SourceHost::default());
        session
            .eval(
                r#"
                import { marker as first, factory, update_marker } from "components.mms"
                import { marker as second } from "components.mms"
                import ast { marker as template } from "components.mms"
                update_marker()
            "#,
            )
            .unwrap();
        let first = session.scopes[0].get("first").cloned().unwrap();
        let second = session.scopes[0].get("second").cloned().unwrap();
        let template = session.scopes[0].get("template").cloned().unwrap();
        let Value::ComponentObject { id: first, .. } = first else {
            panic!("ordinary import was not live")
        };
        let Value::ComponentObject { id: second, .. } = second else {
            panic!("ordinary import was not live")
        };
        assert!(
            matches!(template, Value::ComponentExpr(_)),
            "import ast was not deferred"
        );
        let Some(Value::ComponentObject {
            id: factory_one, ..
        }) = session.eval("factory()").unwrap().value
        else {
            panic!("factory did not return a live component");
        };
        let Some(Value::ComponentObject {
            id: factory_two, ..
        }) = session.eval("factory()").unwrap().value
        else {
            panic!("factory did not return a live component");
        };
        assert_eq!(
            first, second,
            "ordinary imports must reuse the direct export identity"
        );
        assert_ne!(
            factory_one, factory_two,
            "factory calls must create fresh live components"
        );
        assert_eq!(session.host().registers, 3);
        assert_eq!(
            session.host().method_calls,
            1,
            "module closure captured the live marker"
        );
    }
}
