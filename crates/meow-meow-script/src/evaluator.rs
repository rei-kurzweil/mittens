use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::ast::{
    BinOpKind, BlockStatement, ComponentExpression, ElseBranch, Expression, Statement, UnaryOpKind,
};
use crate::host::{CallbackHandle, ComponentHandle, Host, HostContext, HostError, HostErrorKind, HostRequest, HostResponse, TransportValue};
use crate::object::{CeChild, HeapHandle, MaterializedCE, Object, ObjectId, RuntimeClosure, Value};
use crate::runtime::{api_key, Catalog, ValueSignature};
use crate::{MeowMeowParser, MeowMeowTokenizer};

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
        }
    }

    pub(crate) fn for_session(
        host: &'a mut H,
        scopes: Vec<HashMap<String, Value>>,
        heap: HeapHandle,
        callbacks: HashMap<CallbackHandle, Value>,
        catalog: Arc<Catalog>,
        context: &'a mut HostContext,
    ) -> Self {
        Self { host, scopes, emitted: vec![], heap, callbacks, catalog: Some(catalog), context: Some(context) }
    }

    pub(crate) fn into_session_state(self) -> (Vec<HashMap<String, Value>>, HashMap<CallbackHandle, Value>) {
        (self.scopes, self.callbacks)
    }

    pub fn evaluate(&mut self, source: &str) -> Result<Evaluation, EvalError> {
        let tokens = MeowMeowTokenizer::new(source)
            .tokenize()
            .map_err(|e| EvalError::Tokenize(format!("{e:?}")))?;
        let parser = if let Some(catalog) = &self.catalog {
            MeowMeowParser::with_component_names(tokens, catalog.components.keys().cloned())
        } else { MeowMeowParser::new(tokens) };
        let statements = parser
            .parse_program()
            .map_err(|e| EvalError::Parse(e.message))?;
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
                    Value::Object(id) => id.with_map_mut(|map| map.insert(key, value))
                        .ok_or_else(|| EvalError::Runtime("stale table reference".into()))
                        .map(|_| Flow::Continue),
                    _ => Err(EvalError::Runtime("index reassignment requires a table".into())),
                }
            }
            Statement::Reassign { .. } => Err(EvalError::Runtime(
                "only identifier reassignment is supported by the pure evaluator".into(),
            )),
            Statement::Return(value) => Ok(Flow::Return(match &value.value {
                Some(expression) => self.eval_expr(expression)?,
                None => Value::Null,
            })),
            Statement::Expression(expression) => {
                let value = self.eval_expr(expression)?;
                if let Value::ComponentExpr(tree) = value {
                    let tree = *tree;
                    self.emit_component(tree.clone())?;
                    self.emitted.push(tree);
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
            Statement::Import { .. } => Err(EvalError::Runtime(
                "filesystem imports require a capable host".into(),
            )),
        }
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
                let map = fields.iter()
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
            | (Value::Object(id), Value::Identifier(key)) => id.with_map(|values| values.get(&key).cloned().unwrap_or(Value::Null))
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
                    Flow::Return(v) => Ok(v),
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
                    if let Some(catalog) = &self.catalog {
                        let spec = catalog.components.get(&component_type.to_lowercase())
                            .ok_or_else(|| unknown("component", &component_type, catalog.components.keys().map(String::as_str)))?;
                        let signature = spec.methods.get(&method).ok_or_else(|| unknown("component method", &method, spec.methods.keys().map(String::as_str)))?;
                        validate_args(&format!("{component_type}.{method}"), signature, &args)?;
                    }
                    match self.dispatch(HostRequest::InvokeComponentMethod {
                        component: id,
                        component_type,
                        method,
                        args,
                    })? {
                        HostResponse::Value(value) => Ok(value),
                        HostResponse::Unit => Ok(Value::Null),
                        HostResponse::Transport(value) => transport_to_value(value),
                        response => component_response(response, "Component".into()),
                    }
                }
                Value::Map(map) => Ok(map.get(&method).cloned().unwrap_or(Value::Null)),
                Value::Object(id) => id.with_map(|map| map.get(&method).cloned().unwrap_or(Value::Null))
                    .ok_or_else(|| EvalError::Runtime("stale table reference".into())),
                Value::Identifier(name) if name == "Math" => math(&method, &args),
                Value::Identifier(namespace) if self.catalog.as_ref().is_some_and(|c| c.has_namespace(&namespace)) => {
                    self.call_api(Some(&namespace), &method, args)
                }
                Value::Identifier(name) if self.catalog.is_some() => {
                    let catalog = self.catalog.as_ref().unwrap();
                    let suggestion = catalog.components.keys()
                        .min_by_key(|candidate| edit_distance(&candidate.to_lowercase(), &name.to_lowercase()));
                    let suffix = suggestion.map_or(String::new(), |candidate| format!("; did you mean '{candidate}'?"));
                    Err(EvalError::Runtime(format!("unknown component or namespace '{name}'{suffix}")))
                }
                other => Err(EvalError::Runtime(format!(
                    "cannot call method '{method}' on {other:?}"
                ))),
            };
        }
        if let Expression::Identifier(name) = callee {
            if self.catalog.as_ref().is_some_and(|c| c.apis.contains_key(&api_key(None, &name.0))) {
                return self.call_api(None, &name.0, args);
            }
        }
        Err(EvalError::Runtime("value is not callable".into()))
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
        let mut tree = MaterializedCE {
            component_type: self.catalog.as_ref().map_or_else(
                || component.component_type.0.clone(),
                |catalog| catalog.components.get(&component.component_type.0.to_lowercase())
                    .map(|spec| spec.name.clone()).unwrap_or_else(|| component.component_type.0.clone())),
            component_property_assignment_only: false,
            ctor_method: first.as_ref().map(|(method, _)| method.clone()),
            ctor_args: first.map(|(_, args)| args).unwrap_or_default(),
            calls: constructors.into_iter().skip(1).collect(),
            named: Vec::new(),
            positionals: Vec::new(),
            deferred_block: None,
            children: Vec::new(),
        };
        for statement in &component.body.statements {
            match statement {
                Statement::Reassign {
                    target: Expression::Identifier(name),
                    value,
                } => tree.named.push((name.0.clone(), self.eval_expr(value)?)),
                Statement::Expression(Expression::Component(child)) => {
                    tree.children.push(CeChild::Spawn(self.materialize(child)?))
                }
                Statement::Expression(expression) => {
                    let value = self.eval_expr(expression)?;
                    match value {
                        Value::ComponentExpr(child) => tree.children.push(CeChild::Spawn(*child)),
                        Value::ComponentObject { id, .. } => {
                            tree.children.push(CeChild::Attach(id))
                        }
                        Value::String(_) => tree.positionals.push(value),
                        _ => {}
                    }
                }
                _ => {
                    tree.deferred_block = Some(RuntimeClosure {
                        body: component.body.clone(),
                        captured_env: Arc::new(self.snapshot()),
                        heap: HeapHandle::new(),
                        analysis: None,
                    });
                    break;
                }
            }
        }
        if let Some(catalog) = &self.catalog {
            let spec = catalog.components.get(&component.component_type.0.to_lowercase())
                .ok_or_else(|| unknown("component", &component.component_type.0, catalog.components.keys().map(String::as_str)))?.clone();
            if let Some(method) = &tree.ctor_method {
                let signature = spec.constructors.get(method).ok_or_else(|| unknown("constructor", method, spec.constructors.keys().map(String::as_str)))?;
                validate_args(&format!("{}.{}", spec.name, method), signature, &tree.ctor_args)?;
            }
            for (method, args) in &tree.calls {
                let signature = spec.builder_calls.get(method).ok_or_else(|| unknown("builder call", method, spec.builder_calls.keys().map(String::as_str)))?;
                validate_args(&format!("{}.{}", spec.name, method), signature, args)?;
            }
            for (property, value) in &tree.named {
                let ty = spec.properties.get(property).ok_or_else(|| unknown("property", property, spec.properties.keys().map(String::as_str)))?;
                if !ty.accepts(value) { return Err(EvalError::Runtime(format!("property '{}.{}' has the wrong value type", spec.name, property))); }
            }
            for (index, value) in tree.positionals.iter().enumerate() {
                let Some(ty) = spec.positional.get(index) else { return Err(EvalError::Runtime(format!("component '{}' does not accept positional value {}", spec.name, index + 1))); };
                if !ty.accepts(value) { return Err(EvalError::Runtime(format!("positional value {} for '{}' has the wrong type", index + 1, spec.name))); }
            }
            if let Some(normalize) = &spec.normalize { normalize(&mut tree).map_err(EvalError::Runtime)?; }
            if let Some(validate) = &spec.validate { validate(&mut tree).map_err(EvalError::Runtime)?; }
        }
        Ok(tree)
    }

    fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, EvalError> {
        if let Some(context) = self.context.as_deref_mut() {
            let response = self.host.dispatch_with_context(context, request).map_err(EvalError::from)?;
            adopt_component_response(context, &response);
            Ok(response)
        } else { self.host.dispatch(request).map_err(Into::into) }
    }

    fn emit_component(&mut self, tree: MaterializedCE) -> Result<HostResponse, EvalError> {
        if let Some(context) = self.context.as_deref_mut() {
            let response = self.host.dispatch_with_context(context, HostRequest::Emit { tree })
                .map_err(EvalError::from)?;
            adopt_component_response(context, &response);
            Ok(response)
        } else { self.host.dispatch(HostRequest::Spawn { tree }).map_err(Into::into) }
    }

    fn host_query(&mut self, selector: String, scope: Option<ComponentHandle>, multiple: bool) -> Result<Value, EvalError> {
        let response = self.dispatch(HostRequest::Query { selector, scope, multiple })?;
        response_to_query_value(response)
    }

    fn call_api(&mut self, namespace: Option<&str>, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        let key = api_key(namespace, name);
        let spec = self.catalog.as_ref().and_then(|c| c.apis.get(&key)).cloned()
            .ok_or_else(|| EvalError::Runtime(format!("unknown host API '{key}'")))?;
        validate_args(&spec.id, &spec.signature, &args)?;
        let transport = args.into_iter().map(|value| self.to_transport(value)).collect::<Result<Vec<_>, _>>()?;
        match self.dispatch(HostRequest::CallApi { api_id: spec.id.clone(), args: transport })? {
            HostResponse::Transport(value) => transport_to_value(value),
            HostResponse::Value(value) => Ok(value),
            HostResponse::Unit => Ok(Value::Null),
            other => component_response(other, "API result".into()),
        }
    }

    fn to_transport(&mut self, value: Value) -> Result<TransportValue, EvalError> {
        let mut visiting = HashSet::new();
        self.to_transport_inner(value, &mut visiting)
    }

    fn to_transport_inner(&mut self, value: Value, visiting: &mut HashSet<ObjectId>) -> Result<TransportValue, EvalError> {
        match value {
            Value::Null => Ok(TransportValue::Null), Value::Bool(v) => Ok(TransportValue::Bool(v)),
            Value::Number(v) => Ok(TransportValue::Number(v)), Value::String(v) | Value::Identifier(v) => Ok(TransportValue::String(v)),
            Value::Array(values) => values.into_iter().map(|v| self.to_transport_inner(v, visiting)).collect::<Result<_, _>>().map(TransportValue::Array),
            Value::Map(map) => map.into_iter().map(|(k, v)| Ok((k, self.to_transport_inner(v, visiting)?))).collect::<Result<_, EvalError>>().map(TransportValue::Table),
            Value::Object(id) => {
                if !visiting.insert(id.clone()) {
                    return Err(EvalError::Host(HostError { kind: HostErrorKind::Conversion, operation: "value_conversion".into(), message: "cyclic table cannot cross the host boundary".into() }));
                }
                let map = id.with_map(Clone::clone).ok_or_else(|| EvalError::Runtime("stale table reference".into()))?;
                let converted = map.into_iter().map(|(k, v)| Ok((k, self.to_transport_inner(v, visiting)?))).collect::<Result<_, EvalError>>().map(TransportValue::Table);
                visiting.remove(&id);
                converted
            }
            Value::ComponentObject { id, .. } => Ok(TransportValue::Component(id)),
            function @ Value::Function { .. } => {
                let context = self.context.as_deref_mut().ok_or_else(|| EvalError::Runtime("callbacks require a session".into()))?;
                let handle = context.allocate_callback(); self.callbacks.insert(handle, function); Ok(TransportValue::Callback(handle))
            }
            other => Err(EvalError::Host(HostError { kind: HostErrorKind::Conversion, operation: "value_conversion".into(), message: format!("value {other:?} cannot cross the host boundary") })),
        }
    }

    pub(crate) fn invoke_value(&mut self, callback: Value, args: Vec<Value>) -> Result<Value, EvalError> {
        let Value::Function { params, body, captured_env, .. } = callback else { return Err(EvalError::Runtime("callback is not callable".into())); };
        self.scopes.push((*captured_env).clone());
        self.scopes.push(params.into_iter().enumerate().map(|(i, p)| (p, args.get(i).cloned().unwrap_or(Value::Null))).collect());
        let result = self.eval_block(&body); self.scopes.pop(); self.scopes.pop();
        match result? { Flow::Return(value) => Ok(value), _ => Ok(Value::Null) }
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
        HostResponse::Component { handle, component_type } => Ok(Value::ComponentObject { id: handle, component_type }),
        HostResponse::Components(values) => Ok(Value::Array(values.into_iter().map(|(id, component_type)| Value::ComponentObject { id, component_type }).collect())),
        HostResponse::Value(value) => Ok(value), HostResponse::Transport(value) => transport_to_value(value), HostResponse::Unit => Ok(Value::Null),
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

fn transport_to_value(value: TransportValue) -> Result<Value, EvalError> {
    Ok(match value {
        TransportValue::Null => Value::Null, TransportValue::Bool(v) => Value::Bool(v),
        TransportValue::Number(v) => Value::Number(v), TransportValue::String(v) => Value::String(v),
        TransportValue::Array(values) => Value::Array(values.into_iter().map(transport_to_value).collect::<Result<_, _>>()?),
        TransportValue::Table(values) => Value::Map(values.into_iter().map(|(k, v)| Ok((k, transport_to_value(v)?))).collect::<Result<_, EvalError>>()?),
        TransportValue::Component(id) => Value::ComponentObject { id, component_type: "Component".into() },
        TransportValue::Callback(_) => return Err(EvalError::Runtime("a host cannot return a callback handle as a script closure".into())),
    })
}

fn validate_args(name: &str, signature: &ValueSignature, args: &[Value]) -> Result<(), EvalError> {
    if (!signature.variadic && args.len() != signature.arguments.len()) || (signature.variadic && args.len() < signature.arguments.len()) {
        return Err(EvalError::Runtime(format!("'{name}' expects {} argument(s), got {}", signature.arguments.len(), args.len())));
    }
    for (index, ty) in signature.arguments.iter().enumerate() {
        if !ty.accepts(&args[index]) { return Err(EvalError::Runtime(format!("argument {} to '{name}' has the wrong type", index + 1))); }
    }
    Ok(())
}

fn unknown<'a>(kind: &str, name: &str, known: impl Iterator<Item = &'a str>) -> EvalError {
    let suggestion = known.min_by_key(|candidate| edit_distance(&candidate.to_lowercase(), &name.to_lowercase()));
    let suffix = suggestion.map_or(String::new(), |candidate| format!("; did you mean '{candidate}'?"));
    EvalError::Runtime(format!("unknown {kind} '{name}'{suffix}"))
}

fn edit_distance(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0]; row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let old = row[j + 1]; row[j + 1] = (row[j + 1] + 1).min(row[j] + 1).min(previous + usize::from(ca != cb)); previous = old;
        }
    } *row.last().unwrap()
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
    let nums = args
        .iter()
        .map(|v| {
            if let Value::Number(n) = v {
                Ok(*n)
            } else {
                Err(EvalError::Runtime(format!("Math.{method} expects numbers")))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = match (method, nums.as_slice()) {
        ("sin", [x]) => x.sin(),
        ("cos", [x]) => x.cos(),
        ("tan", [x]) => x.tan(),
        ("sqrt", [x]) => x.sqrt(),
        ("abs", [x]) => x.abs(),
        ("floor", [x]) => x.floor(),
        ("ceil", [x]) => x.ceil(),
        ("round", [x]) => x.round(),
        ("atan", [x]) => x.atan(),
        ("atan2", [y, x]) => y.atan2(*x),
        ("clamp", [x, lo, hi]) => x.clamp(*lo, *hi),
        _ => {
            return Err(EvalError::Runtime(format!(
                "unknown or invalid Math.{method}"
            )));
        }
    };
    Ok(Value::Number(value))
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
