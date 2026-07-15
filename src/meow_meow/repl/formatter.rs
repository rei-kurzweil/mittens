use crate::engine::ecs::World;
use crate::meow_meow::ast::{
    BlockStatement, ComponentExpression, ConstructorCall, Expression, Ident, Statement,
    TableFieldValue,
};
use crate::meow_meow::component_registry::subtree_to_ce_ast;
use crate::meow_meow::object::{BuiltinTableKind, CeChild, MaterializedCE, Value};
use crate::meow_meow::unparser::{unparse_component, unparse_expression};

pub fn format_repl_value(value: &Value, world: &World) -> Result<String, String> {
    match value {
        Value::Identifier(name) if name == "__mms_world__" => Ok("<world>".into()),
        Value::ComponentObject { id, .. } => {
            let ce = subtree_to_ce_ast(world, *id)
                .map_err(|_| format!("stale component handle: component {id:?} is not live"))?;
            Ok(unparse_component(&ce))
        }
        Value::ComponentExpr(ce) => Ok(unparse_component(&materialized_to_ast(ce)?)),
        Value::Array(values) => {
            let items = values
                .iter()
                .map(|v| value_to_expression(v, world))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(unparse_expression(&Expression::Array(items)))
        }
        Value::Map(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let fields = keys
                .into_iter()
                .map(|key| {
                    Ok(TableFieldValue {
                        name: Ident(key.clone()),
                        value: value_to_expression(&map[key], world)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(unparse_expression(&Expression::Table(fields)))
        }
        Value::Object(id) => id
            .with_map(|map| format_repl_value(&Value::Map(map.clone()), world))
            .ok_or_else(|| "<stale object>".to_string())?,
        Value::Function { .. } => Ok("<fn>".into()),
        Value::Module { .. } => Ok("<module>".into()),
        Value::BuiltinTable(BuiltinTableKind::Math) => Ok("<builtin Math>".into()),
        Value::BuiltinTable(BuiltinTableKind::MusicNote) => Ok("<builtin MusicNote>".into()),
        other => Ok(unparse_expression(&value_to_expression(other, world)?)),
    }
}

fn value_to_expression(value: &Value, world: &World) -> Result<Expression, String> {
    Ok(match value {
        Value::Null => Expression::Null,
        Value::Bool(v) => Expression::Bool(*v),
        Value::Number(v) => Expression::Number(*v),
        Value::Dimension { value, unit } => Expression::Dimension(*value, *unit),
        Value::String(v) => Expression::String(v.clone()),
        Value::Identifier(v) => Expression::Identifier(Ident(v.clone())),
        Value::Array(v) => Expression::Array(
            v.iter()
                .map(|x| value_to_expression(x, world))
                .collect::<Result<_, _>>()?,
        ),
        Value::Map(v) => Expression::Table(
            v.iter()
                .map(|(k, x)| {
                    Ok(TableFieldValue {
                        name: Ident(k.clone()),
                        value: value_to_expression(x, world)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        ),
        Value::ComponentObject { id, .. } => Expression::Component(
            subtree_to_ce_ast(world, *id)
                .map_err(|_| format!("stale component handle: component {id:?} is not live"))?,
        ),
        Value::ComponentExpr(ce) => Expression::Component(materialized_to_ast(ce)?),
        other => return Err(format!("value has no MMS source form: {other:?}")),
    })
}

fn materialized_to_ast(ce: &MaterializedCE) -> Result<ComponentExpression, String> {
    let mut constructors = Vec::new();
    if let Some(method) = &ce.ctor_method {
        constructors.push(ConstructorCall {
            method: Ident(method.clone()),
            args: ce
                .ctor_args
                .iter()
                .map(|v| value_to_expression(v, &World::default()))
                .collect::<Result<_, _>>()?,
        });
    }
    let mut body = Vec::new();
    for (method, args) in &ce.calls {
        body.push(Statement::Expression(Expression::Call(
            crate::meow_meow::ast::CallExpression {
                callee: Box::new(Expression::Identifier(Ident(method.clone()))),
                args: args
                    .iter()
                    .map(|v| value_to_expression(v, &World::default()))
                    .collect::<Result<_, _>>()?,
            },
        )));
    }
    for (name, value) in &ce.named {
        body.push(Statement::Reassign {
            target: Expression::Identifier(Ident(name.clone())),
            value: value_to_expression(value, &World::default())?,
        });
    }
    for value in &ce.positionals {
        body.push(Statement::Expression(value_to_expression(
            value,
            &World::default(),
        )?));
    }
    if let Some(deferred) = &ce.deferred_block {
        body.extend(deferred.body.statements.clone());
    }
    for child in &ce.children {
        match child {
            CeChild::Spawn(child) => body.push(Statement::Expression(Expression::Component(
                materialized_to_ast(child)?,
            ))),
            CeChild::Attach(id) => {
                return Err(format!(
                    "detached component expression references live component {id:?}"
                ));
            }
        }
    }
    Ok(ComponentExpression {
        component_type: Ident(ce.component_type.clone()),
        constructors,
        body: BlockStatement { statements: body },
    })
}
