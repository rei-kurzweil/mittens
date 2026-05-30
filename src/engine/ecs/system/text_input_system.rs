use crate::engine::ecs::component::{TextComponent, TextInputComponent, TransformComponent};
use crate::engine::ecs::rx::TextInputCaretDirection;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::engine::user_input::{InputState, TextInputFrameEvent};

#[derive(Debug, Default)]
pub struct TextInputSystem {
    focused: Option<ComponentId>,
    handlers_installed: bool,
}

const OWNED_TEXT_INPUT_CONTENT_LABEL: &str = "__text_input_content";
const OWNED_TEXT_INPUT_TEXT_LABEL: &str = "__text_input_text";

impl TextInputSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_handlers(&mut self, rx: &mut RxWorld) {
        if self.handlers_installed {
            return;
        }
        self.handlers_installed = true;

        rx.add_global_handler_closure(SignalKind::Click, move |world, emit, env| {
            let Some(EventSignal::Click { renderable, .. }) = env.event.as_ref() else {
                return;
            };

            if let Some(component_id) = nearest_text_input_ancestor(world, *renderable) {
                emit.push_intent_now(
                    component_id,
                    IntentValue::TextInputSetFocus { component_id },
                );
            } else {
                emit.push_intent_now(env.scope, IntentValue::TextInputClearFocus);
            }
        });
    }

    pub fn register_text_input(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        component: ComponentId,
    ) {
        let text = world
            .get_component_by_id_as::<TextInputComponent>(component)
            .map(|input| input.text.clone());
        let Some(text) = text else {
            return;
        };

        let target = ensure_text_target(world, emit, component)
            .or_else(|| resolve_text_target(world, component));

        if let Some(target) = target {
            emit.push_intent_now(
                component,
                IntentValue::SetText {
                    component_ids: vec![target],
                    text,
                },
            );
        }
    }

    pub fn tick_with_queue(
        &mut self,
        world: &World,
        input: &InputState,
        emit: &mut dyn SignalEmitter,
    ) {
        let Some(focused) = self.focused else {
            return;
        };
        if world.get_component_record(focused).is_none() {
            self.focused = None;
            return;
        }

        for event in input.text_input_events() {
            match event {
                TextInputFrameEvent::InsertText(text) if !text.is_empty() => {
                    emit.push_intent_now(
                        focused,
                        IntentValue::TextInputInsertText { text: text.clone() },
                    );
                }
                TextInputFrameEvent::Backspace => {
                    emit.push_intent_now(focused, IntentValue::TextInputBackspace);
                }
                TextInputFrameEvent::DeleteForward => {
                    emit.push_intent_now(focused, IntentValue::TextInputDeleteForward);
                }
                TextInputFrameEvent::MoveCaretLeft => {
                    emit.push_intent_now(
                        focused,
                        IntentValue::TextInputMoveCaret {
                            direction: TextInputCaretDirection::Left,
                            amount: 1,
                        },
                    );
                }
                TextInputFrameEvent::MoveCaretRight => {
                    emit.push_intent_now(
                        focused,
                        IntentValue::TextInputMoveCaret {
                            direction: TextInputCaretDirection::Right,
                            amount: 1,
                        },
                    );
                }
                TextInputFrameEvent::InsertText(_) => {}
            }
        }
    }

    pub fn clear_focus_if_removed(&mut self, removed_nodes: &[ComponentId]) {
        if self.focused.is_some_and(|focused| removed_nodes.contains(&focused)) {
            self.focused = None;
        }
    }

    pub fn execute_intent(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        env: &Signal,
    ) {
        let Some(intent) = env.intent.as_ref() else {
            return;
        };

        match &intent.value {
            IntentValue::TextInputSetFocus { component_id } => {
                self.set_focus(world, emit, *component_id);
            }
            IntentValue::TextInputClearFocus => {
                self.clear_focus(world, emit, env.scope);
            }
            IntentValue::TextInputInsertText { text } => {
                self.apply_text_edit(world, emit, env.scope, |input| {
                    if input.read_only || text.is_empty() {
                        return false;
                    }
                    let byte = char_to_byte_index(&input.text, input.caret);
                    input.text.insert_str(byte, text);
                    input.caret += text.chars().count();
                    true
                });
            }
            IntentValue::TextInputBackspace => {
                self.apply_text_edit(world, emit, env.scope, |input| {
                    if input.read_only || input.caret == 0 {
                        return false;
                    }
                    let end = char_to_byte_index(&input.text, input.caret);
                    let start = char_to_byte_index(&input.text, input.caret - 1);
                    input.text.replace_range(start..end, "");
                    input.caret -= 1;
                    true
                });
            }
            IntentValue::TextInputDeleteForward => {
                self.apply_text_edit(world, emit, env.scope, |input| {
                    if input.read_only {
                        return false;
                    }
                    let char_count = input.text.chars().count();
                    if input.caret >= char_count {
                        return false;
                    }
                    let start = char_to_byte_index(&input.text, input.caret);
                    let end = char_to_byte_index(&input.text, input.caret + 1);
                    input.text.replace_range(start..end, "");
                    true
                });
            }
            IntentValue::TextInputMoveCaret { direction, amount } => {
                self.move_caret(world, env.scope, *direction, *amount);
            }
            _ => {}
        }
    }

    fn set_focus(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        component_id: ComponentId,
    ) {
        if world
            .get_component_by_id_as::<TextInputComponent>(component_id)
            .is_none()
        {
            return;
        }

        let old = self.focused;
        if old == Some(component_id) {
            if let Some(input) = world.get_component_by_id_as_mut::<TextInputComponent>(component_id)
            {
                input.focused = true;
            }
            return;
        }

        if let Some(old_id) = old {
            if let Some(old_input) = world.get_component_by_id_as_mut::<TextInputComponent>(old_id)
            {
                old_input.focused = false;
            }
        }
        if let Some(new_input) = world.get_component_by_id_as_mut::<TextInputComponent>(component_id)
        {
            new_input.focused = true;
        }
        self.focused = Some(component_id);

        emit.push_event(
            component_id,
            EventSignal::TextInputFocusChanged {
                old,
                new: Some(component_id),
            },
        );
    }

    fn clear_focus(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scope: ComponentId,
    ) {
        let old = self.focused.take();
        let Some(old_id) = old else {
            return;
        };
        if let Some(old_input) = world.get_component_by_id_as_mut::<TextInputComponent>(old_id) {
            old_input.focused = false;
        }
        emit.push_event(
            scope,
            EventSignal::TextInputFocusChanged {
                old: Some(old_id),
                new: None,
            },
        );
    }

    fn move_caret(
        &mut self,
        world: &mut World,
        scope: ComponentId,
        direction: TextInputCaretDirection,
        amount: usize,
    ) {
        let Some(focused) = self.focused.filter(|focused| *focused == scope) else {
            return;
        };
        if let Some(input) = world.get_component_by_id_as_mut::<TextInputComponent>(focused) {
            let char_count = input.text.chars().count();
            match direction {
                TextInputCaretDirection::Left => input.caret = input.caret.saturating_sub(amount),
                TextInputCaretDirection::Right => {
                    input.caret = (input.caret + amount).min(char_count)
                }
            }
        }
    }

    fn apply_text_edit(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scope: ComponentId,
        mut edit: impl FnMut(&mut TextInputComponent) -> bool,
    ) {
        let Some(focused) = self.focused.filter(|focused| *focused == scope) else {
            return;
        };

        let (changed, text, caret) = {
            let Some(input) = world.get_component_by_id_as_mut::<TextInputComponent>(focused) else {
                return;
            };
            let changed = edit(input);
            input.clamp_caret();
            (changed, input.text.clone(), input.caret)
        };

        if !changed {
            return;
        }

        if let Some(target) = resolve_text_target(world, focused) {
            emit.push_intent_now(
                focused,
                IntentValue::SetText {
                    component_ids: vec![target],
                    text: text.clone(),
                },
            );
        }
        emit.push_event(
            focused,
            EventSignal::TextInputChanged {
                component_id: focused,
                text,
                caret,
            },
        );
    }
}

fn resolve_text_target(world: &World, root: ComponentId) -> Option<ComponentId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node != root && world.get_component_by_id_as::<TextComponent>(node).is_some() {
            return Some(node);
        }
        for &child in world.children_of(node).iter().rev() {
            stack.push(child);
        }
    }
    None
}

fn ensure_text_target(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    root: ComponentId,
) -> Option<ComponentId> {
    if let Some(existing) = resolve_text_target(world, root) {
        return Some(existing);
    }

    let content = world.add_component_boxed_named(
        OWNED_TEXT_INPUT_CONTENT_LABEL,
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.2)),
    );
    let text = world.add_component_boxed_named(
        OWNED_TEXT_INPUT_TEXT_LABEL,
        Box::new(TextComponent::new("")),
    );

    let _ = world.add_child(root, content);
    let _ = world.add_child(content, text);
    world.init_component_tree(content, emit);

    Some(text)
}

fn nearest_text_input_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<TextInputComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        if let Some(child_text_input) = world.children_of(node).iter().copied().find(|&child| {
            world
                .get_component_by_id_as::<TextInputComponent>(child)
                .is_some()
        }) {
            return Some(child_text_input);
        }
        cur = world.parent_of(node);
    }
    None
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn focused_text_input_mutates_backing_text() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        let root = world.add_component(TransformComponent::new());
        let input = world.add_component(TextInputComponent::new("hi"));

        let _ = world.add_child(root, input);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        queue.push_intent_now(input, IntentValue::TextInputSetFocus { component_id: input });
        queue.push_intent_now(
            input,
            IntentValue::TextInputInsertText {
                text: "!".to_string(),
            },
        );
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let input_state = world
            .get_component_by_id_as::<TextInputComponent>(input)
            .expect("text input component");
        assert_eq!(input_state.text, "hi!");
        assert_eq!(input_state.caret, 3);

        let text_state = world
            .get_component_by_id_as::<TextComponent>(resolve_text_target(&world, input).expect("spawned backing text"))
            .expect("text component");
        assert_eq!(text_state.text, "hi!");
    }
}