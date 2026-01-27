use crate::engine::ecs;

use super::repl_backend::ReplBackend;
use super::util;

#[derive(Debug, Clone)]
enum PipeValue {
    Id(ecs::ComponentId),
    Node(ecs::component_codec::ComponentDataNode),
}

fn source_ls(backend: &ReplBackend, world: &ecs::World, args: &[&str]) -> Result<Vec<ecs::ComponentId>, String> {
    if !args.is_empty() {
        return Err("ls takes no arguments (in pipes)".to_string());
    }
    Ok(backend.current_listing(world))
}

fn source_cat(backend: &ReplBackend, world: &ecs::World, args: &[&str]) -> Result<Vec<ecs::component_codec::ComponentDataNode>, String> {
    let target = match args {
        [] => backend.cwd(),
        [one] => backend.resolve_path_or_item(world, one)?,
        _ => return Err("cat takes at most one argument (in pipes)".to_string()),
    };

    match target {
        None => {
            let root_ids: Vec<ecs::ComponentId> = world
                .all_components()
                .filter(|&cid| world.parent_of(cid).is_none())
                .collect();

            let mut out = Vec::new();
            for cid in root_ids {
                out.push(ecs::ComponentCodec::encode_subtree_node(world, cid)?);
            }
            Ok(out)
        }
        Some(root) => Ok(vec![ecs::ComponentCodec::encode_subtree_node(world, root)?]),
    }
}

fn stage_grep(
    world: &ecs::World,
    input: Vec<PipeValue>,
    pattern: &str,
) -> Vec<PipeValue> {
    let needle = pattern.to_ascii_lowercase();
    let mut out: Vec<PipeValue> = Vec::new();

    // If the stream contains serialized nodes, grep operates on the serialized tree and outputs
    // entire matching subtrees as JSON.
    let any_nodes = input.iter().any(|v| matches!(v, PipeValue::Node(_)));
    if any_nodes {
        fn node_matches(node: &ecs::component_codec::ComponentDataNode, needle: &str) -> bool {
            if node.name.to_ascii_lowercase().contains(needle) {
                return true;
            }
            if node.type_name.to_ascii_lowercase().contains(needle) {
                return true;
            }
            // Also allow matching on guid string and encoded keys/values.
            if node.guid.to_string().to_ascii_lowercase().contains(needle) {
                return true;
            }
            for (k, v) in &node.data {
                if k.to_ascii_lowercase().contains(needle) {
                    return true;
                }
                if let Some(s) = v.as_str() {
                    if s.to_ascii_lowercase().contains(needle) {
                        return true;
                    }
                }
            }
            false
        }

        fn collect_matching_subtrees(
            node: &ecs::component_codec::ComponentDataNode,
            needle: &str,
            out: &mut Vec<ecs::component_codec::ComponentDataNode>,
        ) {
            if node_matches(node, needle) {
                out.push(node.clone());
                return;
            }
            for child in &node.components {
                collect_matching_subtrees(child, needle, out);
            }
        }

        for value in input {
            let PipeValue::Node(node) = value else {
                continue;
            };

            let mut matches: Vec<ecs::component_codec::ComponentDataNode> = Vec::new();
            collect_matching_subtrees(&node, &needle, &mut matches);

            for m in matches {
                match serde_json::to_string_pretty(&m) {
                    Ok(json) => println!("{}", json),
                    Err(e) => println!("🐈 grep: failed to serialize JSON: {}", e),
                }
                out.push(PipeValue::Node(m));
            }
        }

        return out;
    }

    // Otherwise, grep operates on live components in the world and prints matching properties.
    for (i, value) in input.into_iter().enumerate() {
        let PipeValue::Id(cid) = value else {
            continue;
        };
        let Some(node) = world.get_component_node(cid) else {
            continue;
        };

        let mut matches: Vec<(String, serde_json::Value)> = Vec::new();

        // Treat these as searchable "properties" too.
        let type_name = node.component.name().to_string();
        let meta = [
            ("name".to_string(), serde_json::Value::String(node.name.clone())),
            ("type".to_string(), serde_json::Value::String(type_name.clone())),
            (
                "guid".to_string(),
                serde_json::Value::String(node.guid.to_string()),
            ),
        ];

        for (k, v) in meta {
            let k_l = k.to_ascii_lowercase();
            let v_hit = v
                .as_str()
                .map(|s| s.to_ascii_lowercase().contains(&needle))
                .unwrap_or(false);
            if k_l.contains(&needle) || v_hit {
                matches.push((k, v));
            }
        }

        for (k, v) in node.component.encode() {
            let k_l = k.to_ascii_lowercase();
            let v_hit = v
                .as_str()
                .map(|s| s.to_ascii_lowercase().contains(&needle))
                .unwrap_or(false);

            if k_l.contains(&needle) || v_hit {
                matches.push((k, v));
            }
        }

        if matches.is_empty() {
            continue;
        }

        out.push(PipeValue::Id(cid));

        if let Some(line) = util::format_ls_line(world, i, cid) {
            println!("{}", line);
        }

        for (k, v) in matches {
            // Print full serialized property value.
            println!("🐈   {} = {}", k, v);
        }
    }

    out
}

fn sink_print_summary(world: &ecs::World, items: Vec<PipeValue>) {
    if items.is_empty() {
        println!("🐈 (empty)");
        return;
    }

    for (i, item) in items.into_iter().enumerate() {
        match item {
            PipeValue::Id(cid) => {
                if let Some(line) = util::format_ls_line(world, i, cid) {
                    println!("{}", line);
                }
            }
            PipeValue::Node(node) => {
                println!(
                    "🐈 {}: {}  type={}  guid={}",
                    i, node.name, node.type_name, node.guid
                );
            }
        }
    }
}

/// Execute a command line containing pipes (`|`).
///
/// Piping moves *component objects* (ComponentIds) between stages.
///
/// Supported sources: `ls`, `cat [path]`
/// Supported stages: `grep <pattern>`
/// Supported sinks: trailing `|` (prints ls-style summary)
pub fn try_exec_piped(backend: &mut ReplBackend, world: &ecs::World, cmd: &str) -> Result<bool, String> {
    if !cmd.contains('|') {
        return Ok(false);
    }

    let raw_parts: Vec<&str> = cmd.split('|').collect();
    if raw_parts.len() < 2 {
        return Ok(false);
    }

    let source_part = raw_parts[0].trim();
    if source_part.is_empty() {
        return Err("pipe: missing source command".to_string());
    }

    let mut stage_parts: Vec<&str> = raw_parts[1..].iter().map(|s| s.trim()).collect();

    // Trailing `|` means "print summary".
    let mut has_trailing_sink = false;
    if stage_parts.last().is_some_and(|s| s.is_empty()) {
        has_trailing_sink = true;
        stage_parts.pop();
    }

    let mut src_it = source_part.split_whitespace();
    let Some(src_verb) = src_it.next() else {
        return Err("pipe: missing source verb".to_string());
    };
    let src_args: Vec<&str> = src_it.collect();

    let mut items: Vec<PipeValue> = match src_verb {
        "ls" => source_ls(backend, world, &src_args)?
            .into_iter()
            .map(PipeValue::Id)
            .collect(),
        "cat" => source_cat(backend, world, &src_args)?
            .into_iter()
            .map(PipeValue::Node)
            .collect(),
        _ => {
            return Err(format!(
                "pipe: unsupported source '{}'; try 'ls' or 'cat'",
                src_verb
            ))
        }
    };

    for stage in stage_parts {
        if stage.is_empty() {
            return Err("pipe: empty stage only allowed at end (trailing '|')".to_string());
        }

        let mut it = stage.split_whitespace();
        let Some(verb) = it.next() else {
            return Err("pipe: invalid stage".to_string());
        };

        match verb {
            "grep" => {
                let pattern = it.collect::<Vec<&str>>().join(" ");
                if pattern.trim().is_empty() {
                    return Err("pipe: grep requires a pattern".to_string());
                }
                items = stage_grep(world, items, pattern.trim());
            }
            _ => return Err(format!("pipe: unknown stage '{}'", verb)),
        }
    }

    if has_trailing_sink {
        sink_print_summary(world, items);
    }

    Ok(true)
}
