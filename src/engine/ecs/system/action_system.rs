use crate::engine::ecs::component::{Action, ActionComponent, ActionMethod, ColorComponent, RenderableComponent};
use crate::engine::ecs::{CommandQueue, ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
pub struct ActionSystem;

impl ActionSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_action_component(
        &mut self,
        world: &mut World,
        queue: &mut CommandQueue,
        action: &ActionComponent,
    ) {
        self.execute(world, queue, &action.action);
    }

    pub fn execute(&mut self, world: &mut World, queue: &mut CommandQueue, action: &Action) {
        match &action.method {
            ActionMethod::Noop => {}
            ActionMethod::Print => {
                let msg = action
                    .params
                    .get(0)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                //println!("[ActionSystem] print targets={:?} msg={}", action.target, msg);
            }
            ActionMethod::SetColor => {
                let Some(rgba) = action.params.get(0).and_then(parse_rgba) else {
                    println!("[ActionSystem] set_color: missing/invalid rgba params={:?}", action.params);
                    return;
                };

                let mut color_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_color_targets(world, target, &mut color_cids);
                }
                color_cids.sort();
                color_cids.dedup();

                for color_cid in color_cids {
                    if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
                        c.rgba = rgba;
                        queue.queue_register_color(color_cid);
                    }
                }
            }
            ActionMethod::CommandQueue { command_name } => {
                println!(
                    "[ActionSystem] command_queue '{}' targets={:?} params={:?}",
                    command_name, action.target, action.params
                );
            }
        }
    }
}

impl crate::engine::ecs::system::System for ActionSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // Event-driven: executed by AnimationSystem when keyframes fire.
    }
}

fn parse_rgba(v: &serde_json::Value) -> Option<[f32; 4]> {
    // Accept either a JSON array [r,g,b,a] or object {r,g,b,a} in the future.
    let arr = v.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    let mut rgba = [0.0; 4];
    for i in 0..4 {
        rgba[i] = arr[i].as_f64()? as f32;
    }
    Some(rgba)
}

fn collect_color_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    // 1) Direct ColorComponent target.
    if world.get_component_by_id_as::<ColorComponent>(target).is_some() {
        out.push(target);
        return;
    }

    // 2) RenderableComponent target -> find immediate ColorComponent child.
    if world.get_component_by_id_as::<RenderableComponent>(target).is_some() {
        for &ch in world.children_of(target) {
            if world.get_component_by_id_as::<ColorComponent>(ch).is_some() {
                out.push(ch);
                return;
            }
        }
        return;
    }

    // 3) Generic subtree target (e.g. TransformComponent): search for renderables and their color children.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }

        if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
            for &ch in world.children_of(node) {
                if world.get_component_by_id_as::<ColorComponent>(ch).is_some() {
                    out.push(ch);
                }
            }
        }
    }
}
