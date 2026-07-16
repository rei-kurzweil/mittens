use std::collections::VecDeque;

use crate::engine::{ecs, graphics};
use crate::meow_meow::evaluator::{
    EvalRequest, EvalResponse, HostCallKind, HostValue, MeowMeowEvaluator, MeowMeowEvaluatorHandle,
};
use crate::meow_meow::object::Value;
use crate::meow_meow::repl::{
    MeowMeowReplFrontend, NavigationState, ReplInput, format_repl_value, parse_repl_input,
};

enum ActiveInput {
    Snippet {
        source: String,
    },
    Navigation {
        binding: Option<String>,
        tail: Vec<String>,
    },
}

pub struct MeowMeowRepl {
    frontend: MeowMeowReplFrontend,
    evaluator: MeowMeowEvaluatorHandle,
    pending: VecDeque<String>,
    active: Option<ActiveInput>,
    navigation: NavigationState,
}

impl MeowMeowRepl {
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self {
            frontend: MeowMeowReplFrontend::new()?,
            evaluator: MeowMeowEvaluator::spawn(128),
            pending: VecDeque::new(),
            active: None,
            navigation: NavigationState::new(),
        })
    }

    pub fn sync(
        &mut self,
        world: &mut ecs::World,
        rx: &mut ecs::RxWorld,
        render_assets: &mut graphics::RenderAssets,
        emit: &mut dyn ecs::SignalEmitter,
    ) {
        if let Some(message) = self.navigation.ensure_valid(world) {
            eprintln!("[mms] {message}");
        }
        self.pending.extend(self.frontend.try_recv_all());
        self.start_next(world);
        while let Ok(response) = self.evaluator.responses.pop() {
            match response {
                EvalResponse::HostCall { id, kind } => {
                    let reply = service_host_call(kind, world, rx, render_assets, emit);
                    while self
                        .evaluator
                        .requests
                        .push(EvalRequest::HostCallResult {
                            id,
                            value: reply.clone(),
                        })
                        .is_err()
                    {
                        std::thread::yield_now();
                    }
                }
                EvalResponse::Intent(intent) => {
                    emit.push_intent_now(ecs::ComponentId::default(), intent)
                }
                EvalResponse::SnippetComplete { result } => {
                    let source = match self.active.take() {
                        Some(ActiveInput::Snippet { source }) => source,
                        _ => String::new(),
                    };
                    match result {
                        Ok(Some(Value::Null)) if is_control_call(&source) => {}
                        Ok(Some(value)) => match format_repl_value(&value, world) {
                            Ok(text) => println!("{text}"),
                            Err(error) => eprintln!("[mms] {error}"),
                        },
                        Ok(None) => {}
                        Err(error) => eprintln!("[mms] {error}"),
                    }
                }
                EvalResponse::NavigationComplete { result } => {
                    let (binding, tail) = match self.active.take() {
                        Some(ActiveInput::Navigation { binding, tail }) => (binding, tail),
                        _ => (None, Vec::new()),
                    };
                    match result {
                        Ok(value) => {
                            let previous = self.navigation.clone();
                            let result = self
                                .navigation
                                .set_evaluated(value, binding, world)
                                .and_then(|()| {
                                    for segment in &tail {
                                        self.navigation.cd_child(segment, world)?;
                                    }
                                    Ok(())
                                });
                            if let Err(error) = result {
                                self.navigation = previous;
                                eprintln!("[mms] {error}");
                            }
                        }
                        Err(error) => eprintln!("[mms] {error}"),
                    }
                }
                EvalResponse::ReplReset => self.navigation.reset(),
                EvalResponse::Error { message } => eprintln!("[mms] {message}"),
                EvalResponse::ParsedOk { .. } | EvalResponse::ShutdownAck => {}
            }
        }
        self.start_next(world);
    }

    fn start_next(&mut self, world: &ecs::World) {
        if self.active.is_some() {
            return;
        }
        while let Some(source) = self.pending.pop_front() {
            match parse_repl_input(source) {
                ReplInput::Ls => match self.navigation.listing(world) {
                    Ok(lines) if lines.is_empty() => println!("(empty)"),
                    Ok(lines) => lines.into_iter().for_each(|line| println!("{line}")),
                    Err(error) => eprintln!("[mms] ls: {error}"),
                },
                ReplInput::Pwd => println!("{}", self.navigation.pwd(world)),
                ReplInput::Cd(operand) => {
                    if self.start_cd(operand, world) {
                        return;
                    }
                }
                ReplInput::Snippet(source) => {
                    let request = EvalRequest::EvalSnippet {
                        source: source.clone(),
                        cwd: Some(self.navigation.cwd_value()),
                    };
                    if self.evaluator.requests.push(request).is_ok() {
                        self.active = Some(ActiveInput::Snippet { source });
                    } else {
                        self.pending.push_front(source);
                    }
                    return;
                }
            }
        }
    }

    /// Returns true when an asynchronous expression evaluation was started.
    fn start_cd(&mut self, operand: String, world: &ecs::World) -> bool {
        match operand.as_str() {
            "/" => {
                self.navigation.cd_root();
                return false;
            }
            "." => return false,
            ".." => {
                self.navigation.cd_parent(world);
                return false;
            }
            _ if operand.contains('/') => {
                if let Err(error) = self.navigation.cd_path(&operand, world) {
                    eprintln!("[mms] cd: {error}");
                }
                return false;
            }
            _ => {}
        }

        // Simple names are children first. Dotted identifier chains are a
        // binding anchor followed by deterministic table navigation.
        let identifiers = operand.split('.').collect::<Vec<_>>();
        let is_dotted_path = identifiers.len() > 1
            && identifiers.iter().all(|part| {
                !part.is_empty() && part.chars().all(|c| c == '_' || c.is_alphanumeric())
            });
        if !is_dotted_path && !operand.contains(['(', '[', '{', '"', '\'', ' ']) {
            if self.navigation.cd_child(&operand, world).is_ok() {
                return false;
            }
        }
        let (source, binding, tail) = if is_dotted_path {
            (
                identifiers[0].to_string(),
                Some(identifiers[0].to_string()),
                identifiers[1..]
                    .iter()
                    .map(|part| (*part).to_string())
                    .collect(),
            )
        } else {
            let binding =
                (operand != "cwd" && is_simple_identifier(&operand)).then(|| operand.clone());
            (operand, binding, Vec::new())
        };
        let request = EvalRequest::EvalNavigation {
            source,
            cwd: Some(self.navigation.cwd_value()),
        };
        match self.evaluator.requests.push(request) {
            Ok(()) => {
                self.active = Some(ActiveInput::Navigation { binding, tail });
                true
            }
            Err(_) => false,
        }
    }
}

fn is_simple_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric())
}

fn is_control_call(source: &str) -> bool {
    matches!(
        source.trim().split('(').next().unwrap_or(""),
        "tree" | "dump" | "help" | "clear" | "reset"
    )
}

fn service_host_call(
    kind: HostCallKind,
    world: &mut ecs::World,
    rx: &mut ecs::RxWorld,
    render_assets: &mut graphics::RenderAssets,
    emit: &mut dyn ecs::SignalEmitter,
) -> HostValue {
    match kind {
        HostCallKind::Spawn(ce) => {
            crate::meow_meow::component_registry::with_live_render_assets(render_assets, || {
                crate::meow_meow::component_registry::spawn_tree(&ce, None, world, emit)
            })
            .map(HostValue::ComponentId)
            .unwrap_or_else(|e| {
                eprintln!("[mms] spawn: {e}");
                HostValue::Null
            })
        }
        HostCallKind::Register(ce) => {
            crate::meow_meow::component_registry::with_live_render_assets(render_assets, || {
                crate::meow_meow::component_registry::spawn_tree_uninitialized(&ce, world, emit)
            })
            .map(HostValue::ComponentId)
            .unwrap_or_else(|e| {
                eprintln!("[mms] register: {e}");
                HostValue::Null
            })
        }
        HostCallKind::Attach { parent, child } => {
            if let Some(parent) = parent {
                if let Err(e) = world.add_child(parent, child) {
                    eprintln!("[mms] attach: {e}");
                    return HostValue::Null;
                }
            }
            world.init_component_tree(child, emit);
            HostValue::Null
        }
        HostCallKind::Query {
            selector,
            scope,
            multiple,
        } => {
            if let Some(id) = scope
                && world.get_component_record(id).is_none()
            {
                eprintln!("[mms] stale component handle: component {id:?} is not live");
                return HostValue::Null;
            }
            let roots = scope
                .map(|id| vec![id])
                .unwrap_or_else(|| world.world_roots());
            let mut ids = Vec::new();
            for root in roots {
                if multiple {
                    ids.extend(world.find_all_components(root, &selector));
                } else if let Some(id) = world.find_component(root, &selector) {
                    ids.push(id);
                    break;
                }
            }
            if multiple {
                HostValue::ComponentList(
                    ids.into_iter()
                        .filter_map(|id| {
                            world.component_name(id).map(|name| (id, name.to_string()))
                        })
                        .collect(),
                )
            } else {
                ids.into_iter()
                    .next()
                    .and_then(|id| {
                        world.component_name(id).map(|name| HostValue::Component {
                            id,
                            component_type: name.to_string(),
                        })
                    })
                    .unwrap_or(HostValue::Null)
            }
        }
        HostCallKind::ReplDump { value } => {
            dump_value(&value, world);
            HostValue::Null
        }
        HostCallKind::ReplTree { value, max_depth } => {
            tree_value(&value, world, max_depth.unwrap_or(usize::MAX));
            HostValue::Null
        }
        HostCallKind::ReplHelp => {
            println!("MMS REPL: persistent let bindings, expression echo, and live mutation");
            println!("ls, pwd, cd /, cd .., cd path/to/item, cd <MMS expression>");
            println!(
                "Navigation supports tables, arrays, live components, and detached component expressions."
            );
            println!("cwd is the current value; table fields are implicit after lexical bindings.");
            println!("query(\"#name\"), query_all(world, \"Type\"), component.query_all(\"Text\")");
            println!(
                "At a live component, query(\"Text\") is scoped; query(world, \"Text\") is global."
            );
            println!("tree(value[, max_depth]), dump(value), clear(), reset(), help()");
            HostValue::Null
        }
        HostCallKind::ReplClear => {
            print!("\x1b[2J\x1b[H");
            HostValue::Null
        }
        HostCallKind::RegisterHandler {
            scope,
            signal_kind,
            name,
            handler,
        } => {
            let callback = move |world: &mut ecs::World,
                                 emit: &mut dyn ecs::SignalEmitter,
                                 signal: &ecs::Signal| {
                let arg = crate::meow_meow::runner::event_arg_value(signal);
                if let Err(e) = crate::meow_meow::evaluator::eval_mms_fn(
                    &handler,
                    vec![arg],
                    None,
                    Some(world),
                    Some(emit),
                ) {
                    eprintln!("[mms] handler: {e}");
                }
            };
            if let Some(name) = name {
                rx.add_handler_closure_named(signal_kind, scope, Some(name), callback);
            } else {
                rx.add_handler_closure(signal_kind, scope, callback);
            }
            HostValue::Null
        }
        HostCallKind::AudioClipInstance {
            source,
            start_beat,
            stop_beat,
        } => {
            use crate::engine::ecs::component::AudioClipComponent;
            let Some(src) = world.get_component_by_id_as::<AudioClipComponent>(source) else {
                return HostValue::Null;
            };
            let mut component = AudioClipComponent::instance_of(src);
            if let Some(v) = start_beat {
                component.start_beat = v;
            }
            if let Some(v) = stop_beat {
                component.stop_beat = Some(v);
            }
            HostValue::ComponentId(world.add_component(component))
        }
        HostCallKind::InvokeComponentMethod {
            id,
            component_type,
            method,
            args,
        } => {
            match crate::meow_meow::component_method_registry::invoke_component_method(
                world,
                id,
                &component_type,
                &method,
                &args,
                |intent| emit.push_intent_now(id, intent),
            ) {
                Ok(Value::ComponentObject { id, component_type }) => {
                    HostValue::Component { id, component_type }
                }
                Ok(_) => HostValue::Null,
                Err(e) => {
                    eprintln!("[mms] {e}");
                    HostValue::Null
                }
            }
        }
    }
}

fn selected_ids(value: &Value, world: &ecs::World) -> Result<Vec<ecs::ComponentId>, String> {
    match value {
        Value::Identifier(name) if name == "__mms_world__" => Ok(world.world_roots()),
        Value::ComponentObject { id, .. } if world.get_component_record(*id).is_some() => {
            Ok(vec![*id])
        }
        Value::ComponentObject { id, .. } => Err(format!(
            "stale component handle: component {id:?} is not live"
        )),
        Value::Array(items) => {
            let mut ids = Vec::new();
            for item in items {
                ids.extend(selected_ids(item, world)?);
            }
            Ok(ids)
        }
        other => Err(format!(
            "expected world, component, or component array, got {other:?}"
        )),
    }
}

fn dump_value(value: &Value, world: &ecs::World) {
    match selected_ids(value, world) {
        Ok(ids) => {
            for id in ids {
                match crate::meow_meow::component_registry::subtree_to_ce_ast(world, id) {
                    Ok(ce) => println!("{}", crate::meow_meow::unparser::unparse_component(&ce)),
                    Err(e) => eprintln!("[mms] {e}"),
                }
            }
        }
        Err(_) => match format_repl_value(value, world) {
            Ok(v) => println!("{v}"),
            Err(e) => eprintln!("[mms] {e}"),
        },
    }
}

fn tree_value(value: &Value, world: &ecs::World, max_depth: usize) {
    match selected_ids(value, world) {
        Ok(ids) => {
            for id in ids {
                print_tree(world, id, 0, max_depth);
            }
        }
        Err(e) => eprintln!("[mms] {e}"),
    }
}

fn print_tree(world: &ecs::World, id: ecs::ComponentId, depth: usize, max_depth: usize) {
    let name = world.component_name(id).unwrap_or("<deleted>");
    let authored = world
        .get_component_node(id)
        .map(|node| node.name.as_str())
        .unwrap_or("");
    println!(
        "{}{} {:?}{}",
        "  ".repeat(depth),
        name,
        id,
        if authored.is_empty() || authored == name {
            String::new()
        } else {
            format!(" #{authored}")
        }
    );
    if depth < max_depth {
        for child in world.children_of(id) {
            print_tree(world, *child, depth + 1, max_depth);
        }
    }
}
