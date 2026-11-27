//! `PathGraph` construction and route queries. See
//! `docs/spec/path-finding-system.md`.
use crate::engine::ecs::component::PathGraphComponent;
use crate::engine::ecs::system::path_finding_system::{PathGoal, PathGraph, PathRoute, Rect};
use crate::engine::ecs::{ComponentId, World};
use crate::scripting::object::Value;
use meow_meow_script as mms;
use std::collections::HashMap;

/// `PathGraph.new({ nodes, obstacles, bounds })`.
pub(super) fn create(
    world: &mut World,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    let error = |message: String| format!("PathGraph.new: {message}");
    let (Some("new"), [table]) = (ctor, args) else {
        return Err(error("expected PathGraph.new({ nodes, obstacles, bounds })".into()));
    };
    let fields = match table {
        Value::Map(fields) => fields.clone(),
        Value::Object(id) => id
            .with_map(Clone::clone)
            .ok_or_else(|| error("stale table".into()))?,
        _ => return Err(error("expected a table".into())),
    };
    for key in fields.keys() {
        if !matches!(key.as_str(), "nodes" | "obstacles" | "bounds") {
            return Err(error(format!("unknown field '{key}'")));
        }
    }
    let field = |name: &str| {
        fields
            .get(name)
            .ok_or_else(|| error(format!("missing '{name}'")))
    };
    let rows = |name: &str| -> Result<Vec<Vec<f64>>, String> {
        let Value::Array(rows) = field(name)? else {
            return Err(error(format!("'{name}' must be an array")));
        };
        rows.iter()
            .enumerate()
            .map(|(index, row)| legacy_numbers(row).ok_or_else(|| error(format!("{name}[{index}] must be an array of numbers"))))
            .collect()
    };
    let nodes = rows("nodes")?
        .into_iter()
        .enumerate()
        .map(|(index, row)| sized::<3>(row).ok_or_else(|| error(format!("nodes[{index}] must be [x, y, z]"))))
        .collect::<Result<Vec<_>, _>>()?;
    let obstacles = rows("obstacles")?
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            sized::<4>(row).ok_or_else(|| error(format!("obstacles[{index}] must be [min_x, max_x, min_z, max_z]")))
        })
        .collect::<Result<Vec<Rect>, _>>()?;
    let bounds = legacy_numbers(field("bounds")?)
        .and_then(sized::<4>)
        .ok_or_else(|| error("'bounds' must be [min_x, max_x, min_z, max_z]".into()))?;
    let graph = PathGraph::new(nodes, obstacles, bounds).map_err(|message| error(message))?;
    Ok(world.add_component(PathGraphComponent { graph }))
}

/// `graph.route(here, goal)` and `graph.nearest(here, goals)`.
pub(super) fn invoke(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[mms::Value],
) -> Result<mms::Value, String> {
    let graph = &mut world
        .get_component_by_id_as_mut::<PathGraphComponent>(id)
        .ok_or("stale graph: the PathGraph was removed")?
        .graph;
    match (method, args) {
        ("route", [here, goal]) => {
            let here = point(here).ok_or("here must be [x, y, z]")?;
            let goal = point(goal).ok_or("goal must be [x, y, z]")?;
            Ok(graph
                .route(here, goal)?
                .map_or(mms::Value::Null, |route| result(route, false)))
        }
        ("nearest", [here, mms::Value::Array(goals)]) => {
            let here = point(here).ok_or("here must be [x, y, z]")?;
            let goals = goals
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    path_goal(value).ok_or_else(|| {
                        format!("goals[{index}] must be [x, y, z] or {{ point = [x, y, z] cost = n }}")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(graph
                .nearest(here, &goals)?
                .map_or(mms::Value::Null, |route| result(route, true)))
        }
        ("route", _) => Err("expected route(here, goal)".into()),
        ("nearest", _) => Err("expected nearest(here, goals) with an array of goals".into()),
        _ => Err(format!("unknown method '{method}'")),
    }
}

pub(super) fn configure(
    component: &mut mms::ComponentBuilder<'_, super::runtime_config::MittensBinding>,
    canonical: &'static str,
) {
    use super::runtime_config::{component_signature, host_method};
    use mms::{ValueSignature as S, ValueType as T};
    component.constructor("new", component_signature([T::Any]));
    for name in ["route", "nearest"] {
        host_method(component, canonical, name, S::new(vec![T::Any, T::Any], T::Any));
    }
}

fn result(route: PathRoute, with_index: bool) -> mms::Value {
    let points = route
        .points
        .iter()
        .map(|point| mms::Value::Array(point.iter().map(|&v| mms::Value::Number(v)).collect()))
        .collect();
    let mut fields = HashMap::from([
        ("points".to_string(), mms::Value::Array(points)),
        ("length".to_string(), mms::Value::Number(route.length)),
    ]);
    if with_index {
        fields.insert("index".into(), mms::Value::Number(route.goal as f64));
    }
    mms::Value::Map(fields)
}

fn path_goal(value: &mms::Value) -> Option<PathGoal> {
    let fields = match value {
        mms::Value::Array(_) => return point(value).map(|point| PathGoal { point, cost: 0.0 }),
        mms::Value::Map(fields) => fields.clone(),
        mms::Value::Object(id) => id.with_map(Clone::clone)?,
        _ => return None,
    };
    if fields.keys().any(|key| !matches!(key.as_str(), "point" | "cost")) {
        return None;
    }
    let cost = match fields.get("cost") {
        None | Some(mms::Value::Null) => 0.0,
        Some(mms::Value::Number(cost)) => *cost,
        Some(_) => return None,
    };
    Some(PathGoal {
        point: point(fields.get("point")?)?,
        cost,
    })
}

fn point(value: &mms::Value) -> Option<[f64; 3]> {
    let mms::Value::Array(items) = value else {
        return None;
    };
    let numbers = items
        .iter()
        .map(|item| match item {
            mms::Value::Number(v) => Some(*v),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    sized::<3>(numbers)
}

fn legacy_numbers(value: &Value) -> Option<Vec<f64>> {
    let Value::Array(items) = value else {
        return None;
    };
    items
        .iter()
        .map(|item| match item {
            Value::Number(v) => Some(*v),
            _ => None,
        })
        .collect()
}

fn sized<const N: usize>(values: Vec<f64>) -> Option<[f64; N]> {
    values.try_into().ok()
}

#[cfg(test)]
#[path = "path_graph_api_tests.rs"]
mod tests;
