use crate::engine::ecs::component::{
    ColorComponent, OpacityComponent, RaycastableComponent, RenderableComponent,
    SerializeComponent, TextComponent, TextInputComponent, TextInputGlyphHitComponent,
    TransformComponent,
};
use crate::engine::ecs::rx::TextInputCaretDirection;
use crate::engine::ecs::system::TextSystem;
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
const OWNED_TEXT_INPUT_CARET_BG_LABEL: &str = "__text_input_caret_bg";
const OWNED_TEXT_INPUT_CARET_BG_OPACITY_LABEL: &str = "__text_input_caret_bg_opacity";
const CARET_BG_RGBA: [f32; 4] = [1.0, 0.86, 0.24, 1.0];
const CARET_BG_OPACITY_FOCUSED: f32 = 0.8;
const CARET_BG_OPACITY_HIDDEN: f32 = 0.0;
const CARET_BG_Z: f32 = -0.01;

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

            // 1. Try glyph hit metadata first for caret placement.
            let glyph_hit = world.children_of(*renderable).iter().copied().find_map(|child| {
                world.get_component_by_id_as::<TextInputGlyphHitComponent>(child).copied()
            });

            if let Some(hit) = glyph_hit {
                emit.push_intent_now(
                    hit.text_input_root,
                    IntentValue::TextInputSetFocus {
                        component_id: hit.text_input_root,
                    },
                );
                emit.push_intent_now(
                    hit.text_input_root,
                    IntentValue::TextInputMoveCaretTo {
                        index: hit.char_index,
                    },
                );
                return;
            }

            // 2. Fallback to general text input focus.
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

        let _ = ensure_caret_bg(world, emit, component, target);
        sync_caret_bg(world, emit, component, target);

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
                self.move_caret(world, emit, env.scope, *direction, *amount);
            }
            IntentValue::TextInputMoveCaretTo { index } => {
                self.move_caret_to(world, emit, env.scope, *index);
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
            let target = ensure_text_target(world, emit, component_id)
                .or_else(|| resolve_text_target(world, component_id));
            let _ = ensure_caret_bg(world, emit, component_id, target);
            sync_caret_bg(world, emit, component_id, target);
            return;
        }

        if let Some(old_id) = old {
            if let Some(old_input) = world.get_component_by_id_as_mut::<TextInputComponent>(old_id)
            {
                old_input.focused = false;
            }
            let old_target = resolve_text_target(world, old_id);
            sync_caret_bg(world, emit, old_id, old_target);
        }
        if let Some(new_input) = world.get_component_by_id_as_mut::<TextInputComponent>(component_id)
        {
            new_input.focused = true;
        }
        self.focused = Some(component_id);

        let target = ensure_text_target(world, emit, component_id)
            .or_else(|| resolve_text_target(world, component_id));
        let _ = ensure_caret_bg(world, emit, component_id, target);
        sync_caret_bg(world, emit, component_id, target);

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
        let old_target = resolve_text_target(world, old_id);
        sync_caret_bg(world, emit, old_id, old_target);
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
        emit: &mut dyn SignalEmitter,
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
        let target = resolve_text_target(world, focused);
        sync_caret_bg(world, emit, focused, target);
    }

    fn move_caret_to(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        scope: ComponentId,
        index: usize,
    ) {
        let Some(focused) = self.focused.filter(|focused| *focused == scope) else {
            return;
        };
        if let Some(input) = world.get_component_by_id_as_mut::<TextInputComponent>(focused) {
            let char_count = input.text.chars().count();
            input.caret = index.min(char_count);
        }
        let target = resolve_text_target(world, focused);
        sync_caret_bg(world, emit, focused, target);
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

        let target = resolve_text_target(world, focused);
        if let Some(target) = target {
            emit.push_intent_now(
                focused,
                IntentValue::SetText {
                    component_ids: vec![target],
                    text: text.clone(),
                },
            );
        }
        sync_caret_bg(world, emit, focused, target);
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

fn sync_caret_bg(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    root: ComponentId,
    text_target: Option<ComponentId>,
) {
    let Some((caret_bg, opacity_id, x, y, font_size, visible)) = caret_bg_sync_state(world, root, text_target)
    else {
        return;
    };

    if let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(caret_bg) {
        transform.set_position(emit, x, y, CARET_BG_Z);
        transform.set_scale(emit, font_size, font_size, 1.0);
    }

    if let Some(opacity) = world.get_component_by_id_as_mut::<OpacityComponent>(opacity_id) {
        opacity.opacity = if visible {
            CARET_BG_OPACITY_FOCUSED
        } else {
            CARET_BG_OPACITY_HIDDEN
        };
        emit.push_intent_now(
            opacity_id,
            IntentValue::RegisterOpacity {
                component_ids: vec![opacity_id],
            },
        );
    }
}

fn caret_bg_sync_state(
    world: &World,
    root: ComponentId,
    text_target: Option<ComponentId>,
) -> Option<(ComponentId, ComponentId, f32, f32, f32, bool)> {
    let input = world.get_component_by_id_as::<TextInputComponent>(root)?;
    let text_target = text_target?;
    let caret_bg = resolve_named_descendant(world, root, OWNED_TEXT_INPUT_CARET_BG_LABEL)?;
    let opacity_id = resolve_named_descendant(world, caret_bg, OWNED_TEXT_INPUT_CARET_BG_OPACITY_LABEL)?;
    let text = world.get_component_by_id_as::<TextComponent>(text_target)?;
    let (x, y) = TextSystem::caret_local_position(
        &input.text,
        input.caret,
        text.wrap_at,
        text.word_wrap,
        &text.word_wrap_tokens,
        text.font_size,
    );
    Some((caret_bg, opacity_id, x, y, text.font_size, input.focused))
}

fn resolve_named_descendant(world: &World, root: ComponentId, label: &str) -> Option<ComponentId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node != root && world.component_label(node) == Some(label) {
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
    let raycastable = world.add_component_boxed_named(
        "__text_input_raycastable",
        Box::new(RaycastableComponent::enabled()),
    );

    let _ = world.add_child(root, content);
    let _ = world.add_child(content, text);
    let _ = world.add_child(text, raycastable);
    world.init_component_tree(content, emit);

    Some(text)
}

fn ensure_caret_bg(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    root: ComponentId,
    text_target: Option<ComponentId>,
) -> Option<ComponentId> {
    if let Some(existing) = resolve_named_descendant(world, root, OWNED_TEXT_INPUT_CARET_BG_LABEL) {
        return Some(existing);
    }

    let host = text_target.and_then(|text_target| world.parent_of(text_target)).unwrap_or(root);
    let bg = world.add_component_boxed_named(
        OWNED_TEXT_INPUT_CARET_BG_LABEL,
        Box::new(
            TransformComponent::new()
                .with_position(0.5, -0.5, CARET_BG_Z)
                .with_scale(1.0, 1.0, 1.0),
        ),
    );
    let _ = world.add_child(host, bg);

    let serialize = world.add_component(SerializeComponent::off());
    let _ = world.add_child(bg, serialize);

    let color = world.add_component(ColorComponent { rgba: CARET_BG_RGBA });
    let _ = world.add_child(bg, color);

    let renderable = world.add_component(RenderableComponent::square());
    let _ = world.add_child(color, renderable);

    let opacity = world.add_component_boxed_named(
        OWNED_TEXT_INPUT_CARET_BG_OPACITY_LABEL,
        Box::new(OpacityComponent::new().with_opacity(CARET_BG_OPACITY_HIDDEN)),
    );
    let _ = world.add_child(renderable, opacity);

    world.init_component_tree(bg, emit);
    Some(bg)
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
    use crate::engine::ecs::component::{OpacityComponent, TransformComponent};
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

        let text_target = resolve_text_target(&world, input).expect("spawned backing text");
        let has_raycastable = world.children_of(text_target).iter().copied().any(|child| {
            world
                .get_component_by_id_as::<RaycastableComponent>(child)
                .is_some_and(|raycastable| raycastable.enable)
        });
        assert!(has_raycastable, "text input backing text should be raycastable");

        let caret_bg = resolve_named_descendant(&world, input, OWNED_TEXT_INPUT_CARET_BG_LABEL)
            .expect("text input caret background");
        let caret_bg_transform = world
            .get_component_by_id_as::<TransformComponent>(caret_bg)
            .expect("caret bg transform");
        assert_eq!(caret_bg_transform.transform.translation, [3.5, -0.5, CARET_BG_Z]);

        let caret_bg_opacity = resolve_named_descendant(&world, caret_bg, OWNED_TEXT_INPUT_CARET_BG_OPACITY_LABEL)
            .expect("caret bg opacity");
        let caret_bg_opacity = world
            .get_component_by_id_as::<OpacityComponent>(caret_bg_opacity)
            .expect("caret bg opacity component");
        assert!((caret_bg_opacity.opacity - CARET_BG_OPACITY_FOCUSED).abs() < 1e-6);

        queue.push_intent_now(input, IntentValue::TextInputClearFocus);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        let caret_bg_opacity = resolve_named_descendant(&world, caret_bg, OWNED_TEXT_INPUT_CARET_BG_OPACITY_LABEL)
            .expect("caret bg opacity after clear");
        let caret_bg_opacity = world
            .get_component_by_id_as::<OpacityComponent>(caret_bg_opacity)
            .expect("caret bg opacity component after clear");
        assert!(caret_bg_opacity.opacity.abs() < 1e-6);
        }

        #[test]
        fn text_input_glyph_click_moves_caret() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut queue = CommandQueue::new();

        // 1. Setup TextInput with "hello"
        let root = world.add_component(TransformComponent::new());
        let input = world.add_component(TextInputComponent::new("hello"));
        let _ = world.add_child(root, input);

        world.init_component_tree(root, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        // 2. Identify the 'e' glyph (index 1)
        let text_target = resolve_text_target(&world, input).expect("backing text");
        let mut e_glyph_renderable = None;
        for &t_id in world.children_of(text_target) {
            for &r_id in world.children_of(t_id) {
                if world.get_component_by_id_as::<RenderableComponent>(r_id).is_some() {
                    let hit = world.children_of(r_id).iter().copied().find_map(|child| {
                        world.get_component_by_id_as::<TextInputGlyphHitComponent>(child)
                    });
                    if let Some(hit) = hit {
                        if hit.char_index == 1 {
                            e_glyph_renderable = Some(r_id);
                            break;
                        }
                    }
                }
            }
            if e_glyph_renderable.is_some() {
                break;
            }
        }

        let e_glyph = e_glyph_renderable.expect("found 'e' glyph at index 1");

        // 3. Simulate click on 'e' glyph
        // Handlers must be installed for the click to trigger intents.
        systems.text_input.install_handlers(&mut systems.rx);

        queue.push_event(
            input,
            EventSignal::Click {
                raycaster: input, // doesn't matter for this test
                renderable: e_glyph,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: Some((0.0, 0.0)),
            },
        );

        // process_commands will run the global click handler, which pushes intents,
        // then it will execute those intents.
        systems.process_commands(&mut world, &mut visuals, &mut queue);

        // 4. Verify caret moved to 1
        let input_state = world
            .get_component_by_id_as::<TextInputComponent>(input)
            .expect("text input component");
        assert_eq!(input_state.caret, 1, "Caret should have moved to clicked glyph index 1");
        assert!(input_state.focused, "TextInput should be focused after click");

        // 5. Verify caret background updated position
        let caret_bg = resolve_named_descendant(&world, input, OWNED_TEXT_INPUT_CARET_BG_LABEL)
            .expect("text input caret background");
        let caret_bg_transform = world
            .get_component_by_id_as::<TransformComponent>(caret_bg)
            .expect("caret bg transform");
        // 'e' is at index 1. In monospace 1.0 font size, cursor for index 1 is at x=1.0.
        // TextSystem::caret_local_position("hello", 1, ...) returns (1.0, 0.0) -> centered at (1.5, -0.5)
        assert_eq!(caret_bg_transform.transform.translation, [1.5, -0.5, CARET_BG_Z]);
        }
        }