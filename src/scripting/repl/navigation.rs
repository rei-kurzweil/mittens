use crate::engine::ecs::{ComponentId, World};
use crate::scripting::object::{CeChild, MaterializedCE, Value};
use slotmap::KeyData;

use super::format_repl_value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplInput {
    Snippet(String),
    Ls,
    Pwd,
    Cd(String),
}

pub fn parse_repl_input(source: String) -> ReplInput {
    let trimmed = source.trim();
    match trimmed {
        "ls" => ReplInput::Ls,
        "pwd" => ReplInput::Pwd,
        _ => match trimmed.strip_prefix("cd") {
            Some(rest) if rest.starts_with(char::is_whitespace) && !rest.trim().is_empty() => {
                ReplInput::Cd(rest.trim().to_string())
            }
            _ => ReplInput::Snippet(source),
        },
    }
}

#[derive(Debug, Clone)]
enum Cursor {
    World,
    Value(Value),
}

#[derive(Debug, Clone)]
enum Anchor {
    World,
    Binding(String),
    Expression,
}

#[derive(Debug, Clone)]
struct Breadcrumb {
    cursor: Cursor,
    segment: String,
}

#[derive(Debug, Clone)]
pub struct NavigationState {
    cursor: Cursor,
    anchor: Anchor,
    breadcrumbs: Vec<Breadcrumb>,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            cursor: Cursor::World,
            anchor: Anchor::World,
            breadcrumbs: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn cwd_value(&self) -> Value {
        match &self.cursor {
            Cursor::World => Value::Identifier("__mms_world__".into()),
            Cursor::Value(value) => value.clone(),
        }
    }

    pub fn ensure_valid(&mut self, world: &World) -> Option<String> {
        let Cursor::Value(Value::ComponentObject { id, .. }) = &self.cursor else {
            return None;
        };
        if world.get_component_node(*id).is_some() {
            return None;
        }
        let message =
            format!("stale component handle: component {id:?} is not live; returned to /");
        self.reset();
        Some(message)
    }

    pub fn set_evaluated(
        &mut self,
        value: Value,
        binding: Option<String>,
        world: &World,
    ) -> Result<(), String> {
        ensure_navigable(&value, world)?;
        self.cursor = Cursor::Value(value);
        self.anchor = binding.map(Anchor::Binding).unwrap_or(Anchor::Expression);
        self.breadcrumbs.clear();
        Ok(())
    }

    pub fn cd_root(&mut self) {
        self.reset();
    }

    pub fn cd_parent(&mut self, world: &World) {
        if let Cursor::Value(Value::ComponentObject { id, .. }) = &self.cursor {
            if let Some(parent) = world.parent_of(*id) {
                self.cursor = Cursor::Value(component_value(world, parent));
            } else {
                self.reset();
            }
            return;
        }
        if let Some(parent) = self.breadcrumbs.pop() {
            self.cursor = parent.cursor;
        } else if !matches!(self.cursor, Cursor::World) {
            self.reset();
        }
    }

    pub fn cd_child(&mut self, segment: &str, world: &World) -> Result<(), String> {
        let (value, label) = resolve_child(&self.cursor, segment, world)?;
        ensure_navigable(&value, world)?;
        let old = self.cursor.clone();
        self.breadcrumbs.push(Breadcrumb {
            cursor: old,
            segment: label,
        });
        self.cursor = Cursor::Value(value);
        Ok(())
    }

    pub fn cd_path(&mut self, path: &str, world: &World) -> Result<(), String> {
        let previous = self.clone();
        if path.starts_with('/') {
            self.reset();
        }
        for segment in path.split('/').filter(|part| !part.is_empty()) {
            let result = match segment {
                "." => Ok(()),
                ".." => {
                    self.cd_parent(world);
                    Ok(())
                }
                child => self.cd_child(child, world),
            };
            if let Err(error) = result {
                *self = previous;
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn pwd(&self, world: &World) -> String {
        match &self.cursor {
            Cursor::World => "/".into(),
            Cursor::Value(Value::ComponentObject { id, .. }) => live_pwd(world, *id),
            Cursor::Value(_) => {
                let anchor = match &self.anchor {
                    Anchor::World => "/".into(),
                    Anchor::Binding(name) => format!("${name}"),
                    Anchor::Expression => "<expression>".into(),
                };
                let suffix = self
                    .breadcrumbs
                    .iter()
                    .map(|crumb| crumb.segment.as_str())
                    .collect::<Vec<_>>()
                    .join("/");
                if suffix.is_empty() {
                    anchor
                } else {
                    format!("{anchor}/{suffix}")
                }
            }
        }
    }

    pub fn listing(&self, world: &World) -> Result<Vec<String>, String> {
        match &self.cursor {
            Cursor::World => Ok(world
                .world_roots()
                .into_iter()
                .enumerate()
                .filter_map(|(index, id)| {
                    crate::engine::repl::util::format_ls_line(world, index, id)
                })
                .collect()),
            Cursor::Value(Value::ComponentObject { id, .. }) => {
                if world.get_component_node(*id).is_none() {
                    return Err(format!(
                        "stale component handle: component {id:?} is not live"
                    ));
                }
                Ok(world
                    .children_of(*id)
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(index, id)| {
                        crate::engine::repl::util::format_ls_line(world, index, id)
                    })
                    .collect())
            }
            Cursor::Value(value) => value_listing(value, world),
        }
    }
}

fn value_listing(value: &Value, world: &World) -> Result<Vec<String>, String> {
    match value {
        Value::Object(id) => id
            .with_map(|map| table_listing(map, world))
            .ok_or_else(|| String::from("stale evaluated table"))?,
        Value::Map(map) => table_listing(map, world),
        Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| Ok(format!("{index}: {}", format_repl_value(value, world)?)))
            .collect(),
        Value::ComponentExpr(ce) => ce
            .children
            .iter()
            .enumerate()
            .map(|(index, child)| match child {
                CeChild::Spawn(ce) => Ok(format!(
                    "{index}: {}",
                    format_repl_value(&Value::ComponentExpr(Box::new(ce.clone())), world)?
                )),
                CeChild::Attach(id) => Ok(format!("{index}: <live component {id:?}>")),
            })
            .collect(),
        other => Err(format!("unsupported navigation value: {other:?}")),
    }
}

fn table_listing(
    map: &std::collections::HashMap<String, Value>,
    world: &World,
) -> Result<Vec<String>, String> {
    let mut keys = map.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys.into_iter()
        .enumerate()
        .map(|(index, key)| {
            Ok(format!(
                "{index}: {key} = {}",
                format_repl_value(&map[&key], world)?
            ))
        })
        .collect()
}

fn resolve_child(cursor: &Cursor, segment: &str, world: &World) -> Result<(Value, String), String> {
    match cursor {
        Cursor::World => resolve_live(world.world_roots(), segment, world),
        Cursor::Value(Value::ComponentObject { id, .. }) => {
            if world.get_component_node(*id).is_none() {
                return Err(format!(
                    "stale component handle: component {id:?} is not live"
                ));
            }
            resolve_live(world.children_of(*id).to_vec(), segment, world)
        }
        Cursor::Value(Value::Object(id)) => id
            .with_map(|map| resolve_table(map, segment))
            .ok_or_else(|| String::from("stale evaluated table"))?,
        Cursor::Value(Value::Map(map)) => resolve_table(map, segment),
        Cursor::Value(Value::Array(values)) => {
            let index = segment
                .parse::<usize>()
                .map_err(|_| format!("array index must be numeric: {segment}"))?;
            values
                .get(index)
                .cloned()
                .map(|value| (value, index.to_string()))
                .ok_or_else(|| format!("array index out of range: {index}"))
        }
        Cursor::Value(Value::ComponentExpr(ce)) => resolve_ce_child(ce, segment, world),
        Cursor::Value(other) => Err(format!("unsupported navigation value: {other:?}")),
    }
}

fn resolve_table(
    map: &std::collections::HashMap<String, Value>,
    segment: &str,
) -> Result<(Value, String), String> {
    let mut keys = map.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let key = if let Ok(index) = segment.parse::<usize>() {
        keys.get(index)
            .cloned()
            .ok_or_else(|| format!("table index out of range: {index}"))?
    } else {
        segment.to_string()
    };
    map.get(&key)
        .cloned()
        .map(|value| (value, key.clone()))
        .ok_or_else(|| format!("table field not found: {key}"))
}

fn resolve_ce_child(
    ce: &MaterializedCE,
    segment: &str,
    world: &World,
) -> Result<(Value, String), String> {
    let children = ce
        .children
        .iter()
        .map(|child| match child {
            CeChild::Spawn(ce) => Value::ComponentExpr(Box::new(ce.clone())),
            CeChild::Attach(id) => component_value(world, *id),
        })
        .collect::<Vec<_>>();
    if let Ok(index) = segment.parse::<usize>() {
        return children
            .get(index)
            .cloned()
            .map(|value| (value, ce_child_label(&children[index], index)))
            .ok_or_else(|| format!("component-expression index out of range: {index}"));
    }
    let authored_matches = children
        .iter()
        .enumerate()
        .filter(|(_, value)| match value {
            Value::ComponentExpr(ce) => authored_ce_name(ce).as_deref() == Some(segment),
            _ => false,
        })
        .collect::<Vec<_>>();
    if let [(index, value)] = authored_matches.as_slice() {
        return Ok(((*value).clone(), ce_child_label(value, *index)));
    }
    if authored_matches.len() > 1 {
        return Err(format!(
            "ambiguous component-expression child name: {segment}; use its index"
        ));
    }
    let matches = children
        .iter()
        .enumerate()
        .filter(|(_, value)| match value {
            Value::ComponentExpr(ce) => ce.component_type == segment,
            Value::ComponentObject { component_type, .. } => component_type == segment,
            _ => false,
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [(index, value)] => Ok(((*value).clone(), ce_child_label(value, *index))),
        [] => Err(format!("component-expression child not found: {segment}")),
        _ => Err(format!(
            "ambiguous component-expression child: {segment}; use its index"
        )),
    }
}

fn ce_child_label(value: &Value, index: usize) -> String {
    match value {
        Value::ComponentExpr(ce) => authored_ce_name(ce).unwrap_or_else(|| index.to_string()),
        _ => index.to_string(),
    }
}

fn authored_ce_name(ce: &MaterializedCE) -> Option<String> {
    ce.named.iter().find_map(|(name, value)| {
        (name == "name").then(|| match value {
            Value::String(value) => Some(value.clone()),
            _ => None,
        })?
    })
}

fn resolve_live(
    ids: Vec<ComponentId>,
    segment: &str,
    world: &World,
) -> Result<(Value, String), String> {
    if let Ok(index) = segment.parse::<usize>() {
        let id = ids
            .get(index)
            .copied()
            .ok_or_else(|| format!("component index out of range: {index}"))?;
        return Ok((component_value(world, id), component_label(world, id)));
    }
    let id = if let Ok(guid) = segment.parse::<uuid::Uuid>() {
        ids.iter().copied().find(|id| {
            world
                .get_component_node(*id)
                .is_some_and(|n| n.guid == guid)
        })
    } else if let Some(id) = parse_component_id_short(segment) {
        ids.contains(&id).then_some(id)
    } else {
        let found = ids
            .iter()
            .copied()
            .filter(|id| {
                world
                    .get_component_node(*id)
                    .is_some_and(|node| node.name == segment)
            })
            .collect::<Vec<_>>();
        match found.as_slice() {
            [id] => Some(*id),
            [] => None,
            _ => {
                return Err(format!(
                    "ambiguous component name: {segment}; use index, id, or GUID"
                ));
            }
        }
    };
    let id = id.ok_or_else(|| format!("component child not found: {segment}"))?;
    Ok((component_value(world, id), component_label(world, id)))
}

fn ensure_navigable(value: &Value, world: &World) -> Result<(), String> {
    match value {
        Value::Object(id) if id.with_map(|_| ()).is_some() => Ok(()),
        Value::Map(_) | Value::Array(_) | Value::ComponentExpr(_) => Ok(()),
        Value::ComponentObject { id, .. } if world.get_component_node(*id).is_some() => Ok(()),
        Value::ComponentObject { id, .. } => Err(format!(
            "stale component handle: component {id:?} is not live"
        )),
        other => Err(format!(
            "cd: expected a table, array, live component, or component expression; got {other:?}"
        )),
    }
}

fn component_value(world: &World, id: ComponentId) -> Value {
    Value::ComponentObject {
        id,
        component_type: world.component_name(id).unwrap_or("<deleted>").to_string(),
    }
}

fn component_label(world: &World, id: ComponentId) -> String {
    let name = world.component_label(id).unwrap_or("");
    if name.is_empty() {
        format_component_id_short(id)
    } else {
        name.to_string()
    }
}

fn live_pwd(world: &World, mut id: ComponentId) -> String {
    let mut parts = Vec::new();
    loop {
        let Some(node) = world.get_component_node(id) else {
            return "/".into();
        };
        parts.push(format!("{}:{}", format_component_id_short(id), node.name));
        match world.parent_of(id) {
            Some(parent) => id = parent,
            None => break,
        }
    }
    parts.reverse();
    format!("/{}", parts.join("/"))
}

fn format_component_id_short(id: ComponentId) -> String {
    let debug = format!("{id:?}");
    debug
        .split_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .unwrap_or(&debug)
        .to_string()
}

fn parse_component_id_short(value: &str) -> Option<ComponentId> {
    let (index, version) = value.split_once('v')?;
    let index: u32 = index.parse().ok()?;
    let version: u32 = version.parse().ok()?;
    Some(KeyData::from_ffi((u64::from(version) << 32) | u64::from(index)).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn commands_are_classified_without_claiming_language_calls() {
        assert_eq!(parse_repl_input("ls\n".into()), ReplInput::Ls);
        assert_eq!(parse_repl_input("pwd".into()), ReplInput::Pwd);
        assert_eq!(
            parse_repl_input("cd settings.theme".into()),
            ReplInput::Cd("settings.theme".into())
        );
        assert!(matches!(
            parse_repl_input("ls()".into()),
            ReplInput::Snippet(_)
        ));
        assert!(matches!(
            parse_repl_input("let cd = 1".into()),
            ReplInput::Snippet(_)
        ));
    }

    #[test]
    fn tables_and_arrays_use_breadcrumbs_and_reject_scalar_targets() {
        let world = World::default();
        let value = Value::Map(HashMap::from([(
            "items".into(),
            Value::Array(vec![Value::Map(HashMap::from([(
                "name".into(),
                Value::String("cat".into()),
            )]))]),
        )]));
        let mut navigation = NavigationState::new();
        navigation
            .set_evaluated(value, Some("state".into()), &world)
            .unwrap();
        navigation.cd_child("items", &world).unwrap();
        navigation.cd_child("0", &world).unwrap();
        assert_eq!(navigation.pwd(&world), "$state/items/0");
        assert!(navigation.cd_child("name", &world).is_err());
        assert_eq!(navigation.pwd(&world), "$state/items/0");
        navigation.cd_parent(&world);
        assert_eq!(navigation.pwd(&world), "$state/items");
    }
}
