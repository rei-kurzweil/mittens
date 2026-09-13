use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::ast::{
    BinOpKind, BlockStatement, ComponentExpression, ElseBranch, Expression, ImportItem, Statement,
    UnaryOpKind,
};
use crate::block_effect_analyzer::BlockEffectAnalyzer;
use crate::host::{
    CallbackHandle, ComponentHandle, Host, HostContext, HostError, HostErrorKind, HostRequest,
    HostResponse, TransportValue,
};
use crate::object::{
    CeChild, HeapHandle, MaterializedCE, MaterializedConstructor, MaterializedOperation,
    MaterializedProperty, Object, ObjectId, RuntimeClosure, Value,
};
use crate::runtime::{
    Catalog, ComponentBodyMode, ComponentNamePolicy, ComponentOperationIds, ComponentSpec,
    SignalDeclaration, ValueSignature, api_key,
};
use crate::{MeowMeowParser, MeowMeowTokenizer, SourceId};

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    Tokenize(String),
    Parse(String),
    Runtime(String),
    Host(HostError),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tokenize(message) => write!(f, "tokenize error: {message}"),
            Self::Parse(message) => write!(f, "parse error: {message}"),
            Self::Runtime(message) => write!(f, "runtime error: {message}"),
            Self::Host(error) => write!(f, "host error: {error}"),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<HostError> for EvalError {
    fn from(value: HostError) -> Self {
        Self::Host(value)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluation {
    pub value: Option<Value>,
    pub emitted: Vec<MaterializedCE>,
}

enum Flow {
    Continue,
    Return(Value),
    Break,
    LoopContinue,
}

/// Host-neutral synchronous evaluator. All effects leave the language through
/// `Host::dispatch`; the evaluator never imports an engine type.
pub struct Evaluator<'a, H: Host> {
    host: &'a mut H,
    scopes: Vec<HashMap<String, Value>>,
    emitted: Vec<MaterializedCE>,
    heap: HeapHandle,
    callbacks: HashMap<CallbackHandle, Value>,
    catalog: Option<Arc<Catalog>>,
    context: Option<&'a mut HostContext>,
    source_id: Option<SourceId>,
    module_cache: HashMap<SourceId, Value>,
    module_depth: usize,
}

impl<'a, H: Host> Evaluator<'a, H> {
    pub fn new(host: &'a mut H) -> Self {
        let mut root = HashMap::new();
        root.insert("null".into(), Value::Null);
        Self {
            host,
            scopes: vec![root],
            emitted: Vec::new(),
            heap: HeapHandle::new(),
            callbacks: HashMap::new(),
            catalog: None,
            context: None,
            source_id: None,
            module_cache: HashMap::new(),
            module_depth: 0,
        }
    }

    pub(crate) fn for_session(
        host: &'a mut H,
        scopes: Vec<HashMap<String, Value>>,
        heap: HeapHandle,
        callbacks: HashMap<CallbackHandle, Value>,
        catalog: Arc<Catalog>,
        context: &'a mut HostContext,
        source_id: Option<SourceId>,
        module_cache: HashMap<SourceId, Value>,
    ) -> Self {
        Self {
            host,
            scopes,
            emitted: vec![],
            heap,
            callbacks,
            catalog: Some(catalog),
            context: Some(context),
            source_id,
            module_cache,
            module_depth: 0,
        }
    }

    pub(crate) fn for_materialization(host: &'a mut H, catalog: Arc<Catalog>) -> Self {
        Self {
            host,
            scopes: vec![HashMap::from([("null".into(), Value::Null)])],
            emitted: Vec::new(),
            heap: HeapHandle::new(),
            callbacks: HashMap::new(),
            catalog: Some(catalog),
            context: None,
            source_id: None,
            module_cache: HashMap::new(),
            module_depth: 0,
        }
    }

    pub(crate) fn into_session_state(
        self,
    ) -> (
        Vec<HashMap<String, Value>>,
        HashMap<CallbackHandle, Value>,
        HashMap<SourceId, Value>,
    ) {
        (self.scopes, self.callbacks, self.module_cache)
    }

    pub fn evaluate(&mut self, source: &str) -> Result<Evaluation, EvalError> {
        let statements = self.parse(source)?;
        let mut value = None;
        for statement in &statements {
            if let Statement::Expression(expression) = statement {
                let evaluated = self.eval_expr(expression)?;
                if let Value::ComponentExpr(tree) = evaluated {
                    let tree = *tree;
                    let response = self.emit_component(tree.clone())?;
                    value = Some(component_response(response, tree.component_type.clone())?);
                    self.emitted.push(tree);
                } else {
                    if let Value::ComponentObject { id, .. } = &evaluated {
                        self.dispatch(HostRequest::Attach {
                            parent: None,
                            child: *id,
                        })?;
                    }
                    value = Some(evaluated);
                }
            } else if let Flow::Return(returned) = self.eval_statement(statement)? {
                value = Some(returned);
                break;
            }
        }
        Ok(Evaluation {
            value,
            emitted: std::mem::take(&mut self.emitted),
        })
    }

    fn parse(&self, source: &str) -> Result<Vec<Statement>, EvalError> {
        let tokens = MeowMeowTokenizer::new(source)
            .tokenize()
            .map_err(|e| EvalError::Tokenize(format!("{e:?}")))?;
        let parser = if let Some(catalog) = &self.catalog {
            match catalog.component_name_policy {
                ComponentNamePolicy::OpenUppercase => {
                    MeowMeowParser::with_open_component_names_namespaces_and_static_apis(
                        tokens,
                        catalog.components.keys().cloned(),
                        catalog.parser_namespaces(),
                        catalog.static_component_api_paths(),
                    )
                }
                ComponentNamePolicy::StrictRegistered => {
                    MeowMeowParser::with_component_names_namespaces_and_static_apis(
                        tokens,
                        catalog.components.keys().cloned(),
                        catalog.parser_namespaces(),
                        catalog.static_component_api_paths(),
                    )
                }
            }
        } else {
            MeowMeowParser::new(tokens)
        };
        parser
            .parse_program()
            .map_err(|e| EvalError::Parse(e.message))
    }

    fn eval_block(&mut self, block: &BlockStatement) -> Result<Flow, EvalError> {
        self.scopes.push(HashMap::new());
        let result = (|| {
            for statement in &block.statements {
                let flow = self.eval_statement(statement)?;
                if !matches!(flow, Flow::Continue) {
                    return Ok(flow);
                }
            }
            Ok(Flow::Continue)
        })();
        self.scopes.pop();
        result
    }

    fn eval_statement(&mut self, statement: &Statement) -> Result<Flow, EvalError> {
        match statement {
            Statement::Assignment(assignment) => {
                let value = self.eval_expr(&assignment.value)?;
                let value = self.register_live_component_value(value)?;
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert(assignment.name.0.clone(), value);
                Ok(Flow::Continue)
            }
            Statement::Reassign {
                target: Expression::Identifier(name),
                value,
            } => {
                let value = self.eval_expr(value)?;
                let value = self.register_live_component_value(value)?;
                for scope in self.scopes.iter_mut().rev() {
                    if scope.contains_key(&name.0) {
                        scope.insert(name.0.clone(), value);
                        return Ok(Flow::Continue);
                    }
                }
                Err(EvalError::Runtime(format!(
                    "reassignment: '{}' is not defined",
                    name.0
                )))
            }
            Statement::Reassign {
                target: Expression::Index { base, index },
                value,
            } => {
                let base = self.eval_expr(base)?;
                let key = match self.eval_expr(index)? {
                    Value::String(key) | Value::Identifier(key) => key,
                    _ => return Err(EvalError::Runtime("table key must be a string".into())),
                };
                let value = self.eval_expr(value)?;
                match base {
                    Value::Object(id) => id
                        .with_map_mut(|map| map.insert(key, value))
                        .ok_or_else(|| EvalError::Runtime("stale table reference".into()))
                        .map(|_| Flow::Continue),
                    _ => Err(EvalError::Runtime(
                        "index reassignment requires a table".into(),
                    )),
                }
            }
            Statement::Reassign {
                target:
                    Expression::BinaryOp {
                        op: BinOpKind::Dot,
                        lhs,
                        rhs,
                    },
                value,
            } => {
                let base = self.eval_expr(lhs)?;
                let key = match rhs.as_ref() {
                    Expression::Identifier(key) => key.0.clone(),
                    _ => {
                        return Err(EvalError::Runtime(
                            "table field must be an identifier".into(),
                        ));
                    }
                };
                let value = self.eval_expr(value)?;
                match base {
                    Value::Object(id) => id
                        .with_map_mut(|map| map.insert(key, value))
                        .ok_or_else(|| EvalError::Runtime("stale table reference".into()))
                        .map(|_| Flow::Continue),
                    _ => Err(EvalError::Runtime(
                        "field reassignment requires a table".into(),
                    )),
                }
            }
            Statement::Reassign { .. } => {
                Err(EvalError::Runtime("unsupported reassignment target".into()))
            }
            Statement::Return(value) => Ok(Flow::Return(match &value.value {
                Some(expression) => self.eval_expr(expression)?,
                None => Value::Null,
            })),
            Statement::Expression(expression) => {
                let value = self.eval_expr(expression)?;
                if let Value::ComponentExpr(tree) = value {
                    let tree = *tree;
                    if self.module_depth == 0 {
                        self.emit_component(tree.clone())?;
                    }
                    self.emitted.push(tree);
                } else if let Value::ComponentObject { id, .. } = value {
                    self.dispatch(HostRequest::Attach {
                        parent: None,
                        child: id,
                    })?;
                }
                Ok(Flow::Continue)
            }
            Statement::Block(block) => self.eval_block(block),
            Statement::If(statement) => {
                if truthy(&self.eval_expr(&statement.condition)?) {
                    self.eval_block(&statement.then_branch)
                } else if let Some(branch) = &statement.else_branch {
                    match branch {
                        ElseBranch::Block(block) => self.eval_block(block),
                        ElseBranch::If(nested) => {
                            self.eval_statement(&Statement::If((**nested).clone()))
                        }
                    }
                } else {
                    Ok(Flow::Continue)
                }
            }
            Statement::ForIn {
                binding,
                iterable,
                body,
            } => {
                let values = match self.eval_expr(iterable)? {
                    Value::Array(values) => values,
                    other => {
                        return Err(EvalError::Runtime(format!(
                            "for/in expected array, got {other:?}"
                        )));
                    }
                };
                for value in values {
                    self.scopes
                        .push(HashMap::from([(binding.0.clone(), value)]));
                    let flow = self.eval_block(body)?;
                    self.scopes.pop();
                    match flow {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Continue)
            }
            Statement::While { condition, body } => {
                while truthy(&self.eval_expr(condition)?) {
                    match self.eval_block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Continue)
            }
            Statement::Break => Ok(Flow::Break),
            Statement::Continue => Ok(Flow::LoopContinue),
            Statement::Import { ast, items, path } => self.eval_import(*ast, items, path),
        }
    }

    fn eval_import(
        &mut self,
        ast: bool,
        items: &[ImportItem],
        specifier: &str,
    ) -> Result<Flow, EvalError> {
        let response = self.dispatch(HostRequest::LoadSource {
            importer: self.source_id.clone(),
            specifier: specifier.into(),
        })?;
        let HostResponse::Source(loaded) = response else {
            return Err(EvalError::Host(HostError {
                kind: HostErrorKind::InvalidRequest,
                operation: "load_source".into(),
                message: "host returned a non-source response".into(),
            }));
        };
        let mut module = if let Some(module) = self.module_cache.get(&loaded.identity).cloned() {
            module
        } else {
            let identity = loaded.identity.clone();
            let module = self.eval_module_source(&loaded.source, identity.clone())?;
            self.module_cache.insert(identity, module.clone());
            module
        };
        let Value::Module {
            named,
            ast_named,
            sequence,
            live_sequence,
            ..
        } = &module
        else {
            unreachable!("module cache contains only module values")
        };
        let mut live_sequence_updates = Vec::new();
        for item in items {
            let (local, value) = match item {
                ImportItem::Named(name) => (
                    name.0.clone(),
                    if ast {
                        ast_named
                            .get(&name.0)
                            .cloned()
                            .map(|tree| Value::ComponentExpr(Box::new(tree)))
                    } else {
                        named.get(&name.0).cloned()
                    },
                ),
                ImportItem::NamedAlias { name, alias } => (
                    alias.0.clone(),
                    if ast {
                        ast_named
                            .get(&name.0)
                            .cloned()
                            .map(|tree| Value::ComponentExpr(Box::new(tree)))
                    } else {
                        named.get(&name.0).cloned()
                    },
                ),
                ImportItem::PositionalAlias { index, alias } => (
                    alias.0.clone(),
                    if ast {
                        sequence
                            .get(*index)
                            .cloned()
                            .map(|tree| Value::ComponentExpr(Box::new(tree)))
                    } else {
                        live_sequence.get(index).cloned().or_else(|| {
                            sequence
                                .get(*index)
                                .cloned()
                                .map(|tree| Value::ComponentExpr(Box::new(tree)))
                        })
                    },
                ),
            };
            let value = value.ok_or_else(|| {
                if ast {
                    EvalError::Runtime(format!(
                        "import ast item '{local}' from '{specifier}' is not a component template"
                    ))
                } else {
                    EvalError::Runtime(format!(
                        "import item for '{local}' is unavailable from '{specifier}'"
                    ))
                }
            })?;
            let value = if ast {
                match value {
                    Value::ComponentExpr(_) => value,
                    other => {
                        return Err(EvalError::Runtime(format!(
                            "import ast item '{local}' from '{specifier}' is not a component template (got {other:?})"
                        )));
                    }
                }
            } else {
                // Ordinary imports are live. Module assignment already registered
                // named component exports; positional roots are cached below.
                let value = self.register_live_component_value(value)?;
                if let ImportItem::PositionalAlias { index, .. } = item {
                    live_sequence_updates.push((*index, value.clone()));
                }
                value
            };
            self.scopes.last_mut().unwrap().insert(local, value);
        }
        if !live_sequence_updates.is_empty() {
            if let Value::Module { live_sequence, .. } = &mut module {
                for (index, value) in live_sequence_updates {
                    live_sequence.insert(index, value);
                }
            }
            self.module_cache.insert(loaded.identity, module);
        }
        Ok(Flow::Continue)
    }

    fn eval_module_source(&mut self, source: &str, identity: SourceId) -> Result<Value, EvalError> {
        let statements = self.parse(source)?;
        let previous_source = self.source_id.replace(identity);
        self.module_depth += 1;
        self.scopes.push(HashMap::new());
        let emitted_start = self.emitted.len();
        let result = (|| {
            let mut named = HashMap::new();
            let mut ast_named = HashMap::new();
            for statement in &statements {
                if let Statement::Assignment(assignment) = statement
                    && assignment.exported
                {
                    let raw = self.eval_expr(&assignment.value)?;
                    if let Value::ComponentExpr(tree) = &raw {
                        ast_named.insert(assignment.name.0.clone(), (**tree).clone());
                    }
                    let value = self.register_live_component_value(raw)?;
                    self.scopes
                        .last_mut()
                        .unwrap()
                        .insert(assignment.name.0.clone(), value.clone());
                    named.insert(assignment.name.0.clone(), value);
                } else {
                    self.eval_statement(statement)?;
                }
            }
            let sequence = self.emitted.split_off(emitted_start);
            Ok(Value::Module {
                named,
                ast_named,
                sequence,
                live_sequence: HashMap::new(),
                heap: self.heap.clone(),
            })
        })();
        if result.is_err() {
            self.emitted.truncate(emitted_start);
        }
        self.scopes.pop();
        self.module_depth -= 1;
        self.source_id = previous_source;
        result
    }

    fn eval_expr(&mut self, expression: &Expression) -> Result<Value, EvalError> {
        match expression {
            Expression::String(value) => Ok(Value::String(value.clone())),
            Expression::Number(value) => Ok(Value::Number(*value)),
            Expression::Dimension(value, unit) => Ok(Value::Dimension {
                value: *value,
                unit: *unit,
            }),
            Expression::Bool(value) => Ok(Value::Bool(*value)),
            Expression::Null => Ok(Value::Null),
            Expression::Identifier(name) => self
                .lookup(&name.0)
                .cloned()
                .or_else(|| Some(Value::Identifier(name.0.clone())))
                .ok_or_else(|| EvalError::Runtime(format!("unknown identifier '{}'", name.0))),
            Expression::Array(items) => items
                .iter()
                .map(|item| self.eval_expr(item))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Expression::Table(fields) => {
                let map = fields
                    .iter()
                    .map(|field| Ok((field.name.0.clone(), self.eval_expr(&field.value)?)))
                    .collect::<Result<HashMap<_, _>, EvalError>>()?;
                Ok(Value::Object(self.heap.alloc(Object::Map(map))))
            }
            Expression::Index { base, index } => self.eval_index(base, index),
            Expression::UnaryOp { op, operand } => {
                let value = self.eval_expr(operand)?;
                match (op, value) {
                    (UnaryOpKind::Neg, Value::Number(value)) => Ok(Value::Number(-value)),
                    (UnaryOpKind::Not, value) => Ok(Value::Bool(!truthy(&value))),
                    _ => Err(EvalError::Runtime("invalid unary operand".into())),
                }
            }
            Expression::BinaryOp { op, lhs, rhs } => self.eval_binary(op, lhs, rhs),
            Expression::Function { params, body } => Ok(Value::Function {
                params: params.iter().map(|p| p.0.clone()).collect(),
                body: body.clone(),
                captured_env: Arc::new(self.snapshot()),
                heap: self.heap.clone(),
            }),
            Expression::Call(call) => self.eval_call(&call.callee, &call.args),
            Expression::Component(component) => self
                .materialize(component)
                .map(|tree| Value::ComponentExpr(Box::new(tree))),
        }
    }

    fn eval_index(&mut self, base: &Expression, index: &Expression) -> Result<Value, EvalError> {
        let base = self.eval_expr(base)?;
        let index = self.eval_expr(index)?;
        match (base, index) {
            (Value::Array(values), Value::Number(index)) => {
                Ok(values.get(index as usize).cloned().unwrap_or(Value::Null))
            }
            (Value::Map(values), Value::String(key))
            | (Value::Map(values), Value::Identifier(key)) => {
                Ok(values.get(&key).cloned().unwrap_or(Value::Null))
            }
            (Value::Object(id), Value::String(key))
            | (Value::Object(id), Value::Identifier(key)) => id
                .with_map(|values| values.get(&key).cloned().unwrap_or(Value::Null))
                .ok_or_else(|| EvalError::Runtime("stale table reference".into())),
            _ => Err(EvalError::Runtime("invalid index operation".into())),
        }
    }

    fn eval_binary(
        &mut self,
        op: &BinOpKind,
        lhs: &Expression,
        rhs: &Expression,
    ) -> Result<Value, EvalError> {
        if matches!(op, BinOpKind::And) {
            let lhs = self.eval_expr(lhs)?;
            return if truthy(&lhs) {
                self.eval_expr(rhs)
            } else {
                Ok(lhs)
            };
        }
        if matches!(op, BinOpKind::Or) {
            let lhs = self.eval_expr(lhs)?;
            return if truthy(&lhs) {
                Ok(lhs)
            } else {
                self.eval_expr(rhs)
            };
        }
        if matches!(op, BinOpKind::Query) {
            let scope = match self.eval_expr(lhs)? {
                Value::ComponentObject { id, .. } => Some(id),
                Value::Identifier(name) if name == "__mms_world__" || name == "world" => None,
                _ => None,
            };
            let selector = match self.eval_expr(rhs)? {
                Value::String(s) => s,
                other => value_text(&other),
            };
            return match self.dispatch(HostRequest::Query {
                selector,
                scope,
                multiple: false,
            })? {
                HostResponse::Component {
                    handle,
                    component_type,
                } => Ok(Value::ComponentObject {
                    id: handle,
                    component_type,
                }),
                HostResponse::Value(value) => Ok(value),
                HostResponse::Unit => Ok(Value::Null),
                HostResponse::Components(_) => Err(EvalError::Runtime(
                    "query returned multiple components".into(),
                )),
                HostResponse::Transport(value) => transport_to_value(value),
                HostResponse::Source(_) => Err(EvalError::Host(HostError {
                    kind: HostErrorKind::InvalidRequest,
                    operation: "query".into(),
                    message: "host returned source data for a query".into(),
                })),
            };
        }
        if matches!(op, BinOpKind::Dot) {
            let lhs = self.eval_expr(lhs)?;
            let key = match rhs {
                Expression::Identifier(key) => &key.0,
                _ => {
                    return Err(EvalError::Runtime(
                        "table field must be an identifier".into(),
                    ));
                }
            };
            return match lhs {
                Value::Identifier(name) if name == "Math" => match key.as_str() {
                    "pi" => Ok(Value::Number(std::f64::consts::PI)),
                    "e" => Ok(Value::Number(std::f64::consts::E)),
                    _ => Err(EvalError::Runtime(format!("unknown Math constant '{key}'"))),
                },
                Value::Map(values) => Ok(values.get(key).cloned().unwrap_or(Value::Null)),
                Value::Object(id) => id
                    .with_map(|values| values.get(key).cloned().unwrap_or(Value::Null))
                    .ok_or_else(|| EvalError::Runtime("stale table reference".into())),
                other => Err(EvalError::Runtime(format!(
                    "field access requires a table; receiver was {other:?}"
                ))),
            };
        }
        let lhs = self.eval_expr(lhs)?;
        let rhs = self.eval_expr(rhs)?;
        binary_values(op, lhs, rhs)
    }

    fn eval_call(&mut self, callee: &Expression, args: &[Expression]) -> Result<Value, EvalError> {
        let args = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        if let Expression::Identifier(name) = callee {
            match name.0.as_str() {
                "range" => return range(&args),
                "len" => {
                    return Ok(Value::Number(match args.first() {
                        Some(Value::Array(v)) => v.len(),
                        Some(Value::String(v)) => v.chars().count(),
                        Some(Value::Map(v)) => v.len(),
                        Some(Value::Object(id)) => id.with_map(|v| v.len()).unwrap_or(0),
                        _ => 0,
                    } as f64));
                }
                "query" | "query_all" => {
                    let selector = match args.first() {
                        Some(Value::String(s)) => s.clone(),
                        _ => {
                            return Err(EvalError::Runtime(
                                "query expects a selector string".into(),
                            ));
                        }
                    };
                    return self.host_query(selector, None, name.0 == "query_all");
                }
                "on" => return self.register_signal_handler(&args, false),
                "on_global" => return self.register_signal_handler(&args, true),
                _ => {}
            }
            if let Some(Value::Function {
                params,
                body,
                captured_env,
                ..
            }) = self.lookup(&name.0).cloned()
            {
                self.scopes.push((*captured_env).clone());
                self.scopes.push(
                    params
                        .into_iter()
                        .enumerate()
                        .map(|(i, p)| (p, args.get(i).cloned().unwrap_or(Value::Null)))
                        .collect(),
                );
                let result = self.eval_block(&body);
                self.scopes.pop();
                self.scopes.pop();
                return match result? {
                    // A direct factory return is still a live value at the
                    // call boundary.  Promote it here so callbacks capture a
                    // ComponentObject rather than a stale deferred template.
                    Flow::Return(v) => self.register_live_component_value(v),
                    _ => Ok(Value::Null),
                };
            }
        }
        if let Expression::BinaryOp {
            op: BinOpKind::Dot,
            lhs,
            rhs,
        } = callee
        {
            let method = match rhs.as_ref() {
                Expression::Identifier(name) => name.0.clone(),
                _ => return Err(EvalError::Runtime("invalid method name".into())),
            };
            let receiver = self.eval_expr(lhs)?;
            return match receiver {
                Value::ComponentObject { id, component_type } => {
                    if matches!(method.as_str(), "query" | "query_all") {
                        let selector = match args.first() {
                            Some(Value::String(selector)) => selector.clone(),
                            _ => {
                                return Err(EvalError::Runtime(format!(
                                    "{method} expects a selector string"
                                )));
                            }
                        };
                        if args.len() != 1 {
                            return Err(EvalError::Runtime(format!(
                                "{method} expects exactly one selector argument"
                            )));
                        }
                        return self.host_query(selector, Some(id), method == "query_all");
                    }
                    let operation_id = if let Some(catalog) = &self.catalog {
                        let spec = catalog
                            .components
                            .get(&component_type.to_lowercase())
                            .ok_or_else(|| {
                                unknown(
                                    "component",
                                    &component_type,
                                    catalog.components.keys().map(String::as_str),
                                )
                            })?;
                        let signature = spec.methods.get(&method).ok_or_else(|| {
                            unknown(
                                "component method",
                                &method,
                                spec.methods.keys().map(String::as_str),
                            )
                        })?;
                        validate_args(&format!("{component_type}.{method}"), signature, &args)?;
                        catalog
                            .component_operations
                            .get(&component_type.to_lowercase())
                            .and_then(|operations| {
                                operations.methods.get(&method.to_lowercase()).copied()
                            })
                    } else {
                        None
                    };
                    let request = if let Some(operation_id) = operation_id {
                        HostRequest::InvokeComponentMethod {
                            operation_id,
                            component: id,
                            args,
                        }
                    } else {
                        HostRequest::InvokeComponentMethodByName {
                            component: id,
                            component_type,
                            method,
                            args,
                        }
                    };
                    match self.dispatch(request)? {
                        HostResponse::Value(value) => Ok(value),
                        HostResponse::Unit => Ok(Value::Null),
                        HostResponse::Transport(value) => transport_to_value(value),
                        response => component_response(response, "Component".into()),
                    }
                }
                Value::Map(map) => Ok(map.get(&method).cloned().unwrap_or(Value::Null)),
                Value::Object(id) => id
                    .with_map(|map| map.get(&method).cloned().unwrap_or(Value::Null))
                    .ok_or_else(|| EvalError::Runtime("stale table reference".into())),
                Value::Identifier(name) if name == "Math" => math(&method, &args),
                Value::Identifier(namespace)
                    if self
                        .catalog
                        .as_ref()
                        .is_some_and(|c| c.has_namespace(&namespace)) =>
                {
                    self.call_api(Some(&namespace), &method, args)
                }
                Value::Identifier(name) if self.catalog.is_some() => {
                    let catalog = self.catalog.as_ref().unwrap();
                    let suggestion = catalog.components.keys().min_by_key(|candidate| {
                        edit_distance(&candidate.to_lowercase(), &name.to_lowercase())
                    });
                    let suffix = suggestion.map_or(String::new(), |candidate| {
                        format!("; did you mean '{candidate}'?")
                    });
                    Err(EvalError::Runtime(format!(
                        "unknown component or namespace '{name}'{suffix}"
                    )))
                }
                other => Err(EvalError::Runtime(format!(
                    "cannot call method '{method}' on {other:?}"
                ))),
            };
        }
        if let Expression::Identifier(name) = callee {
            if self
                .catalog
                .as_ref()
                .is_some_and(|c| c.apis.contains_key(&api_key(None, &name.0)))
            {
                return self.call_api(None, &name.0, args);
            }
        }
        Err(EvalError::Runtime(format!(
            "value is not callable: {callee:?}"
        )))
    }

    /// In a session, a component expression assigned to a binding denotes one
    /// detached, uninitialized live component. A later component body can
    /// splice that exact handle through `CeChild::Attach`.
    fn register_live_component_value(&mut self, value: Value) -> Result<Value, EvalError> {
        let Value::ComponentExpr(tree) = value else {
            return Ok(value);
        };
        if self.context.is_none() {
            return Ok(Value::ComponentExpr(tree));
        }
        let component_type = tree.component_type.clone();
        component_response(
            self.dispatch(HostRequest::RegisterComponent { tree: *tree })?,
            component_type,
        )
    }

    fn register_signal_handler(
        &mut self,
        args: &[Value],
        global: bool,
    ) -> Result<Value, EvalError> {
        let (scope, signal_index) = if global {
            (None, 0)
        } else {
            let scope = match args.first() {
                Some(Value::ComponentObject { id, .. }) => Some(*id),
                _ => {
                    return Err(EvalError::Runtime(
                        "on expects a live component as argument 0".into(),
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
        match self.dispatch(request)? {
            HostResponse::Unit => Ok(Value::Null),
            other => Err(EvalError::Runtime(format!(
                "signal registration returned unexpected response {other:?}"
            ))),
        }
    }

    pub(crate) fn materialize(
        &mut self,
        component: &ComponentExpression,
    ) -> Result<MaterializedCE, EvalError> {
        let mut constructors = Vec::new();
        for constructor in &component.constructors {
            constructors.push((
                constructor.method.0.clone(),
                constructor
                    .args
                    .iter()
                    .map(|arg| self.eval_expr(arg))
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }
        let first = constructors.first().cloned();
        let component_key = component.component_type.0.to_lowercase();
        let operations = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.component_operations.get(&component_key).cloned());
        let component_spec = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.components.get(&component_key))
            .cloned();
        let body_mode = component_spec
            .as_deref()
            .map_or(ComponentBodyMode::Standard, |spec| spec.body_mode);
        let mut tree = MaterializedCE {
            component_type: self.catalog.as_ref().map_or_else(
                || component.component_type.0.clone(),
                |catalog| {
                    catalog
                        .components
                        .get(&component.component_type.0.to_lowercase())
                        .map(|spec| spec.name.clone())
                        .unwrap_or_else(|| component.component_type.0.clone())
                },
            ),
            component_property_assignment_only: body_mode == ComponentBodyMode::PropsOnly,
            constructor: first.map_or_else(
                || MaterializedConstructor {
                    name: None,
                    operation_id: operations.as_ref().and_then(|ids| ids.default_constructor),
                    arguments: Vec::new(),
                },
                |(name, arguments)| MaterializedConstructor {
                    operation_id: operations
                        .as_ref()
                        .and_then(|ids| ids.constructors.get(&name.to_lowercase()).copied()),
                    name: Some(name),
                    arguments,
                },
            ),
            initializer_calls: constructors
                .into_iter()
                .skip(1)
                .map(|(name, arguments)| MaterializedOperation {
                    operation_id: operations
                        .as_ref()
                        .and_then(|ids| ids.builder_calls.get(&name.to_lowercase()).copied()),
                    name,
                    arguments,
                })
                .collect(),
            properties: Vec::new(),
            positionals: Vec::new(),
            deferred_block: None,
            deferred_callback: None,
            deferred_callback_effect_profile: None,
            children: Vec::new(),
        };
        if body_mode == ComponentBodyMode::Deferred {
            let analysis = BlockEffectAnalyzer::analyze_keyframe_block(&component.body);
            let effect_profile = crate::KeyframeEffectProfile::from(&analysis);
            let closure = RuntimeClosure {
                body: component.body.clone(),
                captured_env: Arc::new(self.snapshot()),
                heap: self.heap.clone(),
                analysis: Some(analysis),
            };
            if let Some(context) = self.context.as_deref_mut() {
                let callback = context.allocate_callback();
                self.callbacks.insert(
                    callback,
                    Value::Function {
                        params: Vec::new(),
                        body: closure.body,
                        captured_env: closure.captured_env,
                        heap: closure.heap,
                    },
                );
                tree.deferred_callback = Some(crate::SessionCallbackRef {
                    session: context.session_handle(),
                    callback,
                });
                tree.deferred_callback_effect_profile = Some(effect_profile);
            } else {
                tree.deferred_block = Some(closure);
            }
        } else {
            let flow = self.eval_component_body_block(
                &component.body,
                &mut tree,
                operations.as_deref(),
                component_spec.as_deref(),
                body_mode,
            )?;
            if matches!(flow, Flow::Break | Flow::LoopContinue) {
                return Err(EvalError::Runtime(
                    "break or continue used outside a component-body loop".into(),
                ));
            }
        }
        if let Some(catalog) = &self.catalog {
            let spec = catalog
                .components
                .get(&component.component_type.0.to_lowercase())
                .cloned();
            let Some(spec) = spec else {
                if catalog.component_name_policy == ComponentNamePolicy::OpenUppercase {
                    return Ok(tree);
                }
                return Err(unknown(
                    "component",
                    &component.component_type.0,
                    catalog.components.keys().map(String::as_str),
                ));
            };
            if let Some(name) = &tree.constructor.name {
                let signature = spec.constructors.get(name).ok_or_else(|| {
                    unknown(
                        "constructor",
                        name,
                        spec.constructors.keys().map(String::as_str),
                    )
                })?;
                validate_args(
                    &format!("{}.{}", spec.name, name),
                    signature,
                    &tree.constructor.arguments,
                )?;
            }
            for call in &tree.initializer_calls {
                let signature = spec.builder_calls.get(&call.name).ok_or_else(|| {
                    unknown(
                        "builder call",
                        &call.name,
                        spec.builder_calls.keys().map(String::as_str),
                    )
                })?;
                validate_args(
                    &format!("{}.{}", spec.name, call.name),
                    signature,
                    &call.arguments,
                )?;
            }
            for property in &tree.properties {
                let ty = spec.properties.get(&property.name).ok_or_else(|| {
                    unknown(
                        "property",
                        &property.name,
                        spec.properties.keys().map(String::as_str),
                    )
                })?;
                if !ty.accepts(&property.value) {
                    return Err(EvalError::Runtime(format!(
                        "property '{}.{}' has the wrong value type",
                        spec.name, property.name
                    )));
                }
            }
            for (index, value) in tree.positionals.iter().enumerate() {
                let Some(ty) = spec.positional.get(index) else {
                    return Err(EvalError::Runtime(format!(
                        "component '{}' does not accept positional value {}",
                        spec.name,
                        index + 1
                    )));
                };
                if !ty.accepts(value) {
                    return Err(EvalError::Runtime(format!(
                        "positional value {} for '{}' has the wrong type",
                        index + 1,
                        spec.name
                    )));
                }
            }
        }
        Ok(tree)
    }

    fn eval_component_body_block(
        &mut self,
        block: &BlockStatement,
        tree: &mut MaterializedCE,
        operations: Option<&ComponentOperationIds>,
        component_spec: Option<&ComponentSpec>,
        body_mode: ComponentBodyMode,
    ) -> Result<Flow, EvalError> {
        self.scopes.push(HashMap::new());
        let result = (|| {
            for statement in &block.statements {
                let flow = self.eval_component_body_statement(
                    statement,
                    tree,
                    operations,
                    component_spec,
                    body_mode,
                )?;
                if !matches!(flow, Flow::Continue) {
                    return Ok(flow);
                }
            }
            Ok(Flow::Continue)
        })();
        self.scopes.pop();
        result
    }

    fn eval_component_body_statement(
        &mut self,
        statement: &Statement,
        tree: &mut MaterializedCE,
        operations: Option<&ComponentOperationIds>,
        component_spec: Option<&ComponentSpec>,
        body_mode: ComponentBodyMode,
    ) -> Result<Flow, EvalError> {
        match statement {
            Statement::Expression(expression) => {
                if let Expression::Call(call) = expression
                    && let Expression::Identifier(method) = call.callee.as_ref()
                    && self.is_component_body_builder_call(&method.0)
                {
                    let args = call
                        .args
                        .iter()
                        .map(|arg| self.eval_expr(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    tree.initializer_calls.push(MaterializedOperation {
                        operation_id: operations.and_then(|ids| {
                            ids.builder_calls.get(&method.0.to_lowercase()).copied()
                        }),
                        name: method.0.clone(),
                        arguments: args,
                    });
                    return Ok(Flow::Continue);
                }

                let value = self.eval_expr(expression)?;
                match value {
                    Value::ComponentExpr(child) => tree.children.push(CeChild::Spawn(*child)),
                    Value::ComponentObject { id, .. } => tree.children.push(CeChild::Attach(id)),
                    Value::String(_) => tree.positionals.push(value),
                    _ => {}
                }
                Ok(Flow::Continue)
            }
            Statement::Reassign {
                target: Expression::Identifier(name),
                value,
            } if self.should_capture_component_property(
                name.0.as_str(),
                component_spec,
                body_mode,
            ) =>
            {
                tree.properties.push(MaterializedProperty {
                    operation_id: operations
                        .and_then(|ids| ids.properties.get(&name.0.to_lowercase()).copied()),
                    name: name.0.clone(),
                    value: self.eval_expr(value)?,
                });
                Ok(Flow::Continue)
            }
            Statement::Assignment(_) | Statement::Reassign { .. } | Statement::Import { .. } => {
                self.eval_statement(statement)
            }
            Statement::Return(value) => Ok(Flow::Return(match &value.value {
                Some(expression) => self.eval_expr(expression)?,
                None => Value::Null,
            })),
            Statement::Block(block) => {
                self.eval_component_body_block(block, tree, operations, component_spec, body_mode)
            }
            Statement::If(statement) => {
                if truthy(&self.eval_expr(&statement.condition)?) {
                    self.eval_component_body_block(
                        &statement.then_branch,
                        tree,
                        operations,
                        component_spec,
                        body_mode,
                    )
                } else if let Some(branch) = &statement.else_branch {
                    match branch {
                        ElseBranch::Block(block) => self.eval_component_body_block(
                            block,
                            tree,
                            operations,
                            component_spec,
                            body_mode,
                        ),
                        ElseBranch::If(nested) => self.eval_component_body_statement(
                            &Statement::If((**nested).clone()),
                            tree,
                            operations,
                            component_spec,
                            body_mode,
                        ),
                    }
                } else {
                    Ok(Flow::Continue)
                }
            }
            Statement::ForIn {
                binding,
                iterable,
                body,
            } => {
                let values = match self.eval_expr(iterable)? {
                    Value::Array(values) => values,
                    other => {
                        return Err(EvalError::Runtime(format!(
                            "for/in expected array, got {other:?}"
                        )));
                    }
                };
                for value in values {
                    self.scopes
                        .push(HashMap::from([(binding.0.clone(), value)]));
                    let flow = self.eval_component_body_block(
                        body,
                        tree,
                        operations,
                        component_spec,
                        body_mode,
                    )?;
                    self.scopes.pop();
                    match flow {
                        Flow::Break => break,
                        Flow::LoopContinue => continue,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                        Flow::Continue => {}
                    }
                }
                Ok(Flow::Continue)
            }
            Statement::While { condition, body } => {
                while truthy(&self.eval_expr(condition)?) {
                    match self.eval_component_body_block(
                        body,
                        tree,
                        operations,
                        component_spec,
                        body_mode,
                    )? {
                        Flow::Break => break,
                        Flow::LoopContinue => continue,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                        Flow::Continue => {}
                    }
                }
                Ok(Flow::Continue)
            }
            Statement::Break => Ok(Flow::Break),
            Statement::Continue => Ok(Flow::LoopContinue),
        }
    }

    fn should_capture_component_property(
        &self,
        name: &str,
        component_spec: Option<&ComponentSpec>,
        body_mode: ComponentBodyMode,
    ) -> bool {
        if body_mode == ComponentBodyMode::PropsOnly {
            return true;
        }
        if matches!(name, "name" | "id" | "guid" | "class") {
            return true;
        }
        // A pre-existing lexical binding remains an ordinary reassignment in
        // structural bodies. Otherwise a declared field is authored component
        // data rather than a new lexical binding.
        if self.lookup(name).is_some() {
            return false;
        }
        component_spec.is_none_or(|component| component.properties.contains_key(name))
    }

    fn is_component_body_builder_call(&self, name: &str) -> bool {
        if self.lookup(name).is_some() || matches!(name, "range" | "len" | "query" | "query_all") {
            return false;
        }
        !self.catalog.as_ref().is_some_and(|catalog| {
            catalog.builtins.contains(name) || catalog.apis.contains_key(&api_key(None, name))
        })
    }

    fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, EvalError> {
        if let Some(context) = self.context.as_deref_mut() {
            let response = self
                .host
                .dispatch_with_context(context, request)
                .map_err(EvalError::from)?;
            adopt_component_response(context, &response);
            Ok(response)
        } else {
            self.host.dispatch(request).map_err(Into::into)
        }
    }

    fn emit_component(&mut self, tree: MaterializedCE) -> Result<HostResponse, EvalError> {
        if let Some(context) = self.context.as_deref_mut() {
            let response = self
                .host
                .dispatch_with_context(context, HostRequest::Emit { tree })
                .map_err(EvalError::from)?;
            adopt_component_response(context, &response);
            Ok(response)
        } else {
            self.host
                .dispatch(HostRequest::Spawn { tree })
                .map_err(Into::into)
        }
    }

    fn host_query(
        &mut self,
        selector: String,
        scope: Option<ComponentHandle>,
        multiple: bool,
    ) -> Result<Value, EvalError> {
        let response = self.dispatch(HostRequest::Query {
            selector,
            scope,
            multiple,
        })?;
        response_to_query_value(response)
    }

    fn call_api(
        &mut self,
        namespace: Option<&str>,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let key = api_key(namespace, name);
        let spec = self
            .catalog
            .as_ref()
            .and_then(|c| c.apis.get(&key))
            .cloned()
            .ok_or_else(|| EvalError::Runtime(format!("unknown host API '{key}'")))?;
        validate_args(&spec.id, &spec.signature, &args)?;
        let transport = args
            .into_iter()
            .map(|value| self.to_transport(value))
            .collect::<Result<Vec<_>, _>>()?;
        let request = if let Some(operation_id) = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.api_operation_ids.get(&key).copied())
        {
            HostRequest::CallApiById {
                operation_id,
                args: transport,
            }
        } else {
            HostRequest::CallApi {
                api_id: spec.id.clone(),
                args: transport,
            }
        };
        match self.dispatch(request)? {
            HostResponse::Transport(value) => self.from_transport(value),
            HostResponse::Value(value) => Ok(value),
            HostResponse::Unit => Ok(Value::Null),
            other => component_response(other, "API result".into()),
        }
    }

    /// Host API tables become ordinary heap-backed MMS tables. Transport is a
    /// snapshot boundary, but a value re-entering this session must retain the
    /// mutation and closure-capture semantics of a table literal.
    fn from_transport(&mut self, value: TransportValue) -> Result<Value, EvalError> {
        Ok(match value {
            TransportValue::Null => Value::Null,
            TransportValue::Bool(value) => Value::Bool(value),
            TransportValue::Number(value) => Value::Number(value),
            TransportValue::String(value) => Value::String(value),
            TransportValue::Array(values) => Value::Array(
                values
                    .into_iter()
                    .map(|value| self.from_transport(value))
                    .collect::<Result<_, _>>()?,
            ),
            TransportValue::Table(values) => {
                let values = values
                    .into_iter()
                    .map(|(key, value)| Ok((key, self.from_transport(value)?)))
                    .collect::<Result<_, EvalError>>()?;
                Value::Object(self.heap.alloc(Object::Map(values)))
            }
            TransportValue::Component(id) => Value::ComponentObject {
                id,
                component_type: "Component".into(),
            },
            TransportValue::Callback(_) => {
                return Err(EvalError::Runtime(
                    "a host cannot return a callback handle as a script closure".into(),
                ));
            }
        })
    }

    fn to_transport(&mut self, value: Value) -> Result<TransportValue, EvalError> {
        let mut visiting = HashSet::new();
        self.to_transport_inner(value, &mut visiting)
    }

    fn to_transport_inner(
        &mut self,
        value: Value,
        visiting: &mut HashSet<ObjectId>,
    ) -> Result<TransportValue, EvalError> {
        match value {
            Value::Null => Ok(TransportValue::Null),
            Value::Bool(v) => Ok(TransportValue::Bool(v)),
            Value::Number(v) => Ok(TransportValue::Number(v)),
            Value::String(v) | Value::Identifier(v) => Ok(TransportValue::String(v)),
            Value::Array(values) => values
                .into_iter()
                .map(|v| self.to_transport_inner(v, visiting))
                .collect::<Result<_, _>>()
                .map(TransportValue::Array),
            Value::Map(map) => map
                .into_iter()
                .map(|(k, v)| Ok((k, self.to_transport_inner(v, visiting)?)))
                .collect::<Result<_, EvalError>>()
                .map(TransportValue::Table),
            Value::Object(id) => {
                if !visiting.insert(id.clone()) {
                    return Err(EvalError::Host(HostError {
                        kind: HostErrorKind::Conversion,
                        operation: "value_conversion".into(),
                        message: "cyclic table cannot cross the host boundary".into(),
                    }));
                }
                let map = id
                    .with_map(Clone::clone)
                    .ok_or_else(|| EvalError::Runtime("stale table reference".into()))?;
                let converted = map
                    .into_iter()
                    .map(|(k, v)| Ok((k, self.to_transport_inner(v, visiting)?)))
                    .collect::<Result<_, EvalError>>()
                    .map(TransportValue::Table);
                visiting.remove(&id);
                converted
            }
            Value::ComponentObject { id, .. } => Ok(TransportValue::Component(id)),
            function @ Value::Function { .. } => {
                let context = self
                    .context
                    .as_deref_mut()
                    .ok_or_else(|| EvalError::Runtime("callbacks require a session".into()))?;
                let handle = context.allocate_callback();
                self.callbacks.insert(handle, function);
                Ok(TransportValue::Callback(handle))
            }
            other => Err(EvalError::Host(HostError {
                kind: HostErrorKind::Conversion,
                operation: "value_conversion".into(),
                message: format!("value {other:?} cannot cross the host boundary"),
            })),
        }
    }

    pub(crate) fn invoke_value(
        &mut self,
        callback: Value,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let Value::Function {
            params,
            body,
            captured_env,
            ..
        } = callback
        else {
            return Err(EvalError::Runtime("callback is not callable".into()));
        };
        self.scopes.push((*captured_env).clone());
        self.scopes.push(
            params
                .into_iter()
                .enumerate()
                .map(|(i, p)| (p, args.get(i).cloned().unwrap_or(Value::Null)))
                .collect(),
        );
        let result = self.eval_block(&body);
        self.scopes.pop();
        self.scopes.pop();
        match result? {
            Flow::Return(value) => self.register_live_component_value(value),
            _ => Ok(Value::Null),
        }
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }
    fn snapshot(&self) -> HashMap<String, Value> {
        self.scopes.iter().flat_map(|scope| scope.clone()).collect()
    }
}

fn response_to_query_value(response: HostResponse) -> Result<Value, EvalError> {
    match response {
        HostResponse::Component {
            handle,
            component_type,
        } => Ok(Value::ComponentObject {
            id: handle,
            component_type,
        }),
        HostResponse::Components(values) => Ok(Value::Array(
            values
                .into_iter()
                .map(|(id, component_type)| Value::ComponentObject { id, component_type })
                .collect(),
        )),
        HostResponse::Value(value) => Ok(value),
        HostResponse::Transport(value) => transport_to_value(value),
        HostResponse::Unit => Ok(Value::Null),
        HostResponse::Source(_) => Err(EvalError::Host(HostError {
            kind: HostErrorKind::InvalidRequest,
            operation: "query".into(),
            message: "host returned source data for a query".into(),
        })),
    }
}

fn adopt_component_response(context: &mut HostContext, response: &HostResponse) {
    match response {
        HostResponse::Component { handle, .. } => context.adopt_component(*handle),
        HostResponse::Components(values) => {
            for (handle, _) in values {
                context.adopt_component(*handle);
            }
        }
        _ => {}
    }
}

pub(crate) fn transport_to_value(value: TransportValue) -> Result<Value, EvalError> {
    Ok(match value {
        TransportValue::Null => Value::Null,
        TransportValue::Bool(v) => Value::Bool(v),
        TransportValue::Number(v) => Value::Number(v),
        TransportValue::String(v) => Value::String(v),
        TransportValue::Array(values) => Value::Array(
            values
                .into_iter()
                .map(transport_to_value)
                .collect::<Result<_, _>>()?,
        ),
        TransportValue::Table(values) => Value::Map(
            values
                .into_iter()
                .map(|(k, v)| Ok((k, transport_to_value(v)?)))
                .collect::<Result<_, EvalError>>()?,
        ),
        TransportValue::Component(id) => Value::ComponentObject {
            id,
            component_type: "Component".into(),
        },
        TransportValue::Callback(_) => {
            return Err(EvalError::Runtime(
                "a host cannot return a callback handle as a script closure".into(),
            ));
        }
    })
}

fn validate_args(name: &str, signature: &ValueSignature, args: &[Value]) -> Result<(), EvalError> {
    let too_few = args.len() < signature.minimum_arguments;
    let too_many = !signature.variadic && args.len() > signature.arguments.len();
    if too_few || too_many {
        let expected = if signature.minimum_arguments == signature.arguments.len() {
            signature.arguments.len().to_string()
        } else {
            format!(
                "{}..={}",
                signature.minimum_arguments,
                signature.arguments.len()
            )
        };
        return Err(EvalError::Runtime(format!(
            "'{name}' expects {expected} argument(s), got {}",
            args.len()
        )));
    }
    for (index, ty) in signature.arguments.iter().take(args.len()).enumerate() {
        if !ty.accepts(&args[index]) {
            return Err(EvalError::Runtime(format!(
                "argument {} to '{name}' has the wrong type: expected {}",
                index + 1,
                ty.name()
            )));
        }
    }
    Ok(())
}

fn unknown<'a>(kind: &str, name: &str, known: impl Iterator<Item = &'a str>) -> EvalError {
    let suggestion = known
        .min_by_key(|candidate| edit_distance(&candidate.to_lowercase(), &name.to_lowercase()));
    let suffix = suggestion.map_or(String::new(), |candidate| {
        format!("; did you mean '{candidate}'?")
    });
    EvalError::Runtime(format!("unknown {kind} '{name}'{suffix}"))
}

fn edit_distance(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(previous + usize::from(ca != cb));
            previous = old;
        }
    }
    *row.last().unwrap()
}

fn component_response(response: HostResponse, component_type: String) -> Result<Value, EvalError> {
    match response {
        HostResponse::Component {
            handle,
            component_type,
        } => Ok(Value::ComponentObject {
            id: handle,
            component_type,
        }),
        HostResponse::Value(value) => Ok(value),
        HostResponse::Unit => Ok(Value::Null),
        HostResponse::Transport(value) => transport_to_value(value),
        HostResponse::Components(mut values) if values.len() == 1 => {
            let (id, ty) = values.remove(0);
            Ok(Value::ComponentObject {
                id,
                component_type: ty,
            })
        }
        _ => Err(EvalError::Runtime(format!(
            "host did not return a component for {component_type}"
        ))),
    }
}

fn range(args: &[Value]) -> Result<Value, EvalError> {
    let (start, end) = match args {
        [Value::Number(end)] => (0, *end as i64),
        [Value::Number(start), Value::Number(end)] => (*start as i64, *end as i64),
        _ => {
            return Err(EvalError::Runtime(
                "range expects one or two numbers".into(),
            ));
        }
    };
    Ok(Value::Array(
        (start..end).map(|n| Value::Number(n as f64)).collect(),
    ))
}

fn math(method: &str, args: &[Value]) -> Result<Value, EvalError> {
    let number = |index: usize| match args.get(index) {
        Some(Value::Number(value)) => Ok(*value),
        _ => Err(EvalError::Runtime(format!(
            "Math.{method}(): argument {index} must be a number"
        ))),
    };
    let require_len = |expected: usize| {
        if args.len() == expected {
            Ok(())
        } else {
            Err(EvalError::Runtime(format!(
                "Math.{method}(): expected {expected} arguments, got {}",
                args.len()
            )))
        }
    };

    match method {
        "sin" | "cos" | "tan" | "sqrt" | "abs" | "floor" | "ceil" | "round" | "atan" => {
            require_len(1)?;
            let value = number(0)?;
            Ok(Value::Number(match method {
                "sin" => value.sin(),
                "cos" => value.cos(),
                "tan" => value.tan(),
                "sqrt" => value.sqrt(),
                "abs" => value.abs(),
                "floor" => value.floor(),
                "ceil" => value.ceil(),
                "round" => value.round(),
                "atan" => value.atan(),
                _ => unreachable!(),
            }))
        }
        "atan2" => {
            require_len(2)?;
            Ok(Value::Number(number(0)?.atan2(number(1)?)))
        }
        "perlin" if matches!(args.len(), 2 | 3) => Ok(Value::Number(crate::math::perlin(
            number(0)?,
            number(1)?,
            if args.len() == 3 {
                Some(number(2)?)
            } else {
                None
            },
        ))),
        "clamp" => {
            require_len(3)?;
            Ok(Value::Number(number(0)?.clamp(number(1)?, number(2)?)))
        }
        "smoothstep" => {
            require_len(3)?;
            let value = number(0)?;
            let edge0 = number(1)?;
            let edge1 = number(2)?;
            if (edge1 - edge0).abs() <= f64::EPSILON {
                return Err(EvalError::Runtime(
                    "Math.smoothstep(): edge0 and edge1 must be distinct".into(),
                ));
            }
            let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
            Ok(Value::Number(t * t * (3.0 - 2.0 * t)))
        }
        "dot" => {
            require_len(2)?;
            let lhs = number_array(&args[0], "Math.dot(): argument 0")?;
            let rhs = number_array(&args[1], "Math.dot(): argument 1")?;
            if lhs.len() != rhs.len() {
                return Err(EvalError::Runtime(format!(
                    "Math.dot(): arrays must have equal length, got {} and {}",
                    lhs.len(),
                    rhs.len()
                )));
            }
            Ok(Value::Number(
                lhs.iter().zip(rhs).map(|(lhs, rhs)| lhs * rhs).sum(),
            ))
        }
        "cross" => {
            require_len(2)?;
            let lhs = number_array(&args[0], "Math.cross(): argument 0")?;
            let rhs = number_array(&args[1], "Math.cross(): argument 1")?;
            if lhs.len() != 3 || rhs.len() != 3 {
                return Err(EvalError::Runtime(
                    "Math.cross(): both arguments must be three-element arrays".into(),
                ));
            }
            Ok(Value::Array(vec![
                Value::Number(lhs[1] * rhs[2] - lhs[2] * rhs[1]),
                Value::Number(lhs[2] * rhs[0] - lhs[0] * rhs[2]),
                Value::Number(lhs[0] * rhs[1] - lhs[1] * rhs[0]),
            ]))
        }
        _ => Err(EvalError::Runtime(format!(
            "unknown or invalid Math.{method}"
        ))),
    }
}

fn number_array(value: &Value, context: &str) -> Result<Vec<f64>, EvalError> {
    let Value::Array(values) = value else {
        return Err(EvalError::Runtime(format!("{context} must be an array")));
    };
    values
        .iter()
        .map(|value| match value {
            Value::Number(value) => Ok(*value),
            _ => Err(EvalError::Runtime(format!(
                "{context} contains a non-number"
            ))),
        })
        .collect()
}

fn binary_values(op: &BinOpKind, lhs: Value, rhs: Value) -> Result<Value, EvalError> {
    match (op, lhs, rhs) {
        (BinOpKind::Add, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
        (BinOpKind::Add, Value::String(a), b) => Ok(Value::String(a + &value_text(&b))),
        (BinOpKind::Add, a, Value::String(b)) => Ok(Value::String(value_text(&a) + &b)),
        (BinOpKind::Sub, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
        (BinOpKind::Mul, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
        (BinOpKind::Div, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a / b)),
        (BinOpKind::Rem, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a % b)),
        (BinOpKind::Eq, a, b) => Ok(Value::Bool(a == b)),
        (BinOpKind::NotEq, a, b) => Ok(Value::Bool(a != b)),
        (BinOpKind::Lt, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),
        (BinOpKind::Gt, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),
        (BinOpKind::LtEq, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)),
        (BinOpKind::GtEq, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)),
        (_, a, b) => Err(EvalError::Runtime(format!(
            "invalid binary operands {a:?} and {b:?}"
        ))),
    }
}

fn truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}
fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) | Value::Identifier(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        other => format!("{other:?}"),
    }
}
