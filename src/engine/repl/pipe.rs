use crate::engine::ecs;
use crate::meow_meow::ast::ComponentExpression;
use crate::meow_meow::component_registry::subtree_to_ce_ast;
use crate::meow_meow::unparser::unparse_component;

use super::repl_backend::ReplBackend;
use super::util;

#[derive(Debug, Clone)]
enum PipeValue {
    Id(ecs::ComponentId),
    Tree(ComponentExpression),
}

fn source_ls(
    backend: &ReplBackend,
    world: &ecs::World,
    args: &[&str],
) -> Result<Vec<ecs::ComponentId>, String> {
    if !args.is_empty() {
        return Err("ls takes no arguments (in pipes)".to_string());
    }
    Ok(backend.current_listing(world))
}

fn source_cat(
    backend: &ReplBackend,
    world: &ecs::World,
    args: &[&str],
) -> Result<Vec<ComponentExpression>, String> {
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
                out.push(subtree_to_ce_ast(world, cid)?);
            }
            Ok(out)
        }
        Some(root) => Ok(vec![subtree_to_ce_ast(world, root)?]),
    }
}

fn stage_grep(world: &ecs::World, input: Vec<PipeValue>, pattern: &str) -> Vec<PipeValue> {
    let needle = pattern.to_ascii_lowercase();
    let mut out: Vec<PipeValue> = Vec::new();

    // If the stream contains serialized trees, grep operates on the MMS source
    // form of each subtree and emits whole matching subtrees.
    let any_trees = input.iter().any(|v| matches!(v, PipeValue::Tree(_)));
    if any_trees {
        fn ce_matches(ce: &ComponentExpression, needle: &str) -> bool {
            // Match against the unparsed MMS text — covers type name,
            // builder names, literal args, and nested children.
            unparse_component(ce)
                .to_ascii_lowercase()
                .contains(needle)
        }

        fn collect_matching(
            ce: &ComponentExpression,
            needle: &str,
            out: &mut Vec<ComponentExpression>,
        ) {
            if ce_matches(ce, needle) {
                out.push(ce.clone());
                return;
            }
            for stmt in &ce.body.statements {
                if let crate::meow_meow::ast::Statement::Expression(
                    crate::meow_meow::ast::Expression::Component(child),
                ) = stmt
                {
                    collect_matching(child, needle, out);
                }
            }
        }

        for value in input {
            let PipeValue::Tree(ce) = value else {
                continue;
            };
            let mut matches: Vec<ComponentExpression> = Vec::new();
            collect_matching(&ce, &needle, &mut matches);
            for m in matches {
                println!("{}", unparse_component(&m));
                out.push(PipeValue::Tree(m));
            }
        }
        return out;
    }

    // Otherwise, grep operates on live components in the world and prints matching props.
    for (i, value) in input.into_iter().enumerate() {
        let PipeValue::Id(cid) = value else {
            continue;
        };
        let Some(node) = world.get_component_node(cid) else {
            continue;
        };

        let type_name = node.component.name().to_string();
        let meta: [(String, String); 3] = [
            ("name".to_string(), node.name.clone()),
            ("type".to_string(), type_name),
            ("guid".to_string(), node.guid.to_string()),
        ];

        let mut matches: Vec<(String, String)> = Vec::new();
        for (k, v) in &meta {
            if k.to_ascii_lowercase().contains(&needle)
                || v.to_ascii_lowercase().contains(&needle)
            {
                matches.push((k.clone(), v.clone()));
            }
        }

        // Match against the unparsed CE form for body content.
        let ce_text = subtree_to_ce_ast(world, cid)
            .map(|ce| unparse_component(&ce))
            .unwrap_or_default();
        if ce_text.to_ascii_lowercase().contains(&needle) {
            matches.push(("mms".to_string(), ce_text));
        }

        if matches.is_empty() {
            continue;
        }
        out.push(PipeValue::Id(cid));
        if let Some(line) = util::format_ls_line(world, i, cid) {
            println!("{}", line);
        }
        for (k, v) in matches {
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
            PipeValue::Tree(ce) => {
                println!("🐈 {}: {}", i, unparse_component(&ce));
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
pub fn try_exec_piped(
    backend: &mut ReplBackend,
    world: &ecs::World,
    cmd: &str,
) -> Result<bool, String> {
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
            .map(PipeValue::Tree)
            .collect(),
        _ => {
            return Err(format!(
                "pipe: unsupported source '{}'; try 'ls' or 'cat'",
                src_verb
            ));
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
