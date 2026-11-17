use crate::engine::user_input::PhysicalKey;

use crate::engine::ecs::{ComponentId, EventSignal, KeyboardEvent, SignalEmitter};
use crate::engine::user_input::{InputProviderId, InputState, KeyboardTransition};

/// Turns ordered platform keyboard records into scene-wide gameplay signals.
#[derive(Debug, Default)]
pub struct KeyboardInputSystem {
    delivered_down: Vec<((InputProviderId, PhysicalKey), KeyboardEvent)>,
    was_captured: bool,
}

impl KeyboardInputSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_input(
        &mut self,
        input: &InputState,
        captured_by_text_input: bool,
        emit: &mut dyn SignalEmitter,
    ) {
        // Removed providers stop contributing immediately. Do not synthesize
        // gameplay key events when a host has no HID sources.
        self.delivered_down
            .retain(|((provider, _), _)| input.provider_is_active(*provider));
        if captured_by_text_input && !self.was_captured {
            for (_, event) in self.delivered_down.drain(..) {
                emit.push_event(ComponentId::default(), EventSignal::KeyUp(event));
            }
        }
        self.was_captured = captured_by_text_input;

        for record in input.keyboard_events() {
            if captured_by_text_input {
                continue;
            }
            let event = KeyboardEvent {
                code: record.code.clone(),
                key: record.key.clone(),
            };
            let source_key = (record.provider_id, record.physical_key);
            match record.transition {
                KeyboardTransition::Down => {
                    if self
                        .delivered_down
                        .iter()
                        .any(|(physical_key, _)| *physical_key == source_key)
                    {
                        continue;
                    }
                    self.delivered_down.push((source_key, event.clone()));
                    emit.push_event(ComponentId::default(), EventSignal::KeyDown(event));
                }
                KeyboardTransition::Press => {
                    // A repeat cannot reactivate gameplay after focus/capture discarded
                    // the corresponding initial down transition.
                    if self
                        .delivered_down
                        .iter()
                        .any(|(physical_key, _)| *physical_key == source_key)
                    {
                        emit.push_event(ComponentId::default(), EventSignal::KeyPress(event));
                    }
                }
                KeyboardTransition::Up => {
                    if let Some(index) = self
                        .delivered_down
                        .iter()
                        .position(|(physical_key, _)| *physical_key == source_key)
                    {
                        let (_, delivered) = self.delivered_down.remove(index);
                        emit.push_event(ComponentId::default(), EventSignal::KeyUp(delivered));
                    }
                }
            }
        }

        // The wheel is gameplay input too, and text entry captures it the same way.
        let (delta_x, delta_y) = input.wheel_delta;
        if !captured_by_text_input && (delta_x != 0.0 || delta_y != 0.0) {
            emit.push_event(ComponentId::default(), EventSignal::MouseWheel { delta_x, delta_y });
        }
        let (delta_x, delta_y) = input.mouse_drag_delta_button(crate::engine::user_input::MouseButton::Right);
        if !captured_by_text_input && (delta_x != 0.0 || delta_y != 0.0) {
            emit.push_event(ComponentId::default(), EventSignal::MouseLook { delta_x, delta_y });
        }
        if !captured_by_text_input {
            use crate::engine::user_input::MouseButton;
            let name = |button: &MouseButton| match button {
                MouseButton::Left => "left",
                MouseButton::Right => "right",
                MouseButton::Middle => "middle",
                MouseButton::Back => "back",
                MouseButton::Forward => "forward",
                MouseButton::Other(_) => "other",
            };
            // The sets carry no order, so `mouse_down` decides it for a button that was both
            // pressed and released this frame: still held means it was released and pressed
            // again (Up, Down); not held means a click inside one frame (Down, Up).
            let up = |button: &MouseButton| EventSignal::MouseButtonUp { button: name(button).into() };
            for button in input.mouse_released.iter().filter(|b| input.mouse_down.contains(b)) {
                emit.push_event(ComponentId::default(), up(button));
            }
            for button in &input.mouse_pressed {
                emit.push_event(ComponentId::default(), EventSignal::MouseButtonDown { button: name(button).into() });
            }
            for button in input.mouse_released.iter().filter(|b| !input.mouse_down.contains(b)) {
                emit.push_event(ComponentId::default(), up(button));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{IntentSignal, SignalEmitter};
    use crate::engine::user_input::KeyCode;
    use crate::engine::user_input::{KeyboardInputRecord, KeyboardTransition};

    #[derive(Default)]
    struct Events(Vec<EventSignal>);

    impl SignalEmitter for Events {
        fn push_event(&mut self, _: ComponentId, event: EventSignal) {
            self.0.push(event);
        }

        fn push_intent(&mut self, _: ComponentId, _: IntentSignal) {}
    }

    fn record(code: KeyCode, key: &str, transition: KeyboardTransition) -> KeyboardInputRecord {
        KeyboardInputRecord {
            provider_id: InputProviderId::MANUAL,
            physical_key: PhysicalKey::Code(code),
            code: Some(
                match code {
                    KeyCode::KeyW => "KeyW",
                    KeyCode::ShiftLeft => "ShiftLeft",
                    _ => unreachable!(),
                }
                .to_string(),
            ),
            key: key.to_string(),
            transition,
        }
    }

    #[test]
    fn preserves_order_across_multiple_transitions_and_repeat() {
        let mut input = InputState::default();
        input.push_keyboard_event(record(
            KeyCode::ShiftLeft,
            "Shift",
            KeyboardTransition::Down,
        ));
        input.push_keyboard_event(record(
            KeyCode::ShiftLeft,
            "Shift",
            KeyboardTransition::Press,
        ));
        input.push_keyboard_event(record(KeyCode::KeyW, "W", KeyboardTransition::Down));
        input.push_keyboard_event(record(KeyCode::KeyW, "W", KeyboardTransition::Press));
        input.push_keyboard_event(record(KeyCode::KeyW, "W", KeyboardTransition::Press));
        // A release may arrive with a different logical value after modifiers/layout change.
        input.push_keyboard_event(record(KeyCode::KeyW, "w", KeyboardTransition::Up));

        let mut system = KeyboardInputSystem::new();
        let mut events = Events::default();
        system.process_input(&input, false, &mut events);

        let kinds: Vec<_> = events.0.iter().map(EventSignal::kind).collect();
        assert_eq!(
            kinds,
            vec![
                crate::engine::ecs::SignalKind::KeyDown,
                crate::engine::ecs::SignalKind::KeyPress,
                crate::engine::ecs::SignalKind::KeyDown,
                crate::engine::ecs::SignalKind::KeyPress,
                crate::engine::ecs::SignalKind::KeyPress,
                crate::engine::ecs::SignalKind::KeyUp,
            ]
        );
        let EventSignal::KeyDown(event) = &events.0[2] else {
            panic!()
        };
        assert_eq!(event.code.as_deref(), Some("KeyW"));
        assert_eq!(event.key, "W");
        let EventSignal::KeyUp(event) = &events.0[5] else {
            panic!()
        };
        assert_eq!(event.key, "W");
    }

    #[test]
    fn capture_releases_delivered_keys_and_requires_a_fresh_down() {
        let mut down = InputState::default();
        down.push_keyboard_event(record(KeyCode::KeyW, "w", KeyboardTransition::Down));
        down.push_keyboard_event(record(KeyCode::KeyW, "w", KeyboardTransition::Press));
        let mut system = KeyboardInputSystem::new();
        let mut events = Events::default();
        system.process_input(&down, false, &mut events);

        system.process_input(&InputState::default(), true, &mut events);
        let mut held_repeat = InputState::default();
        held_repeat.push_keyboard_event(record(KeyCode::KeyW, "w", KeyboardTransition::Press));
        system.process_input(&held_repeat, false, &mut events);

        let kinds: Vec<_> = events.0.iter().map(EventSignal::kind).collect();
        assert_eq!(
            kinds,
            vec![
                crate::engine::ecs::SignalKind::KeyDown,
                crate::engine::ecs::SignalKind::KeyPress,
                crate::engine::ecs::SignalKind::KeyUp,
            ]
        );
    }

    #[test]
    fn scene_reload_does_not_turn_an_already_held_repeat_into_a_press() {
        let mut held_repeat = InputState::default();
        held_repeat.push_keyboard_event(record(KeyCode::KeyW, "w", KeyboardTransition::Press));

        // A new system represents a freshly loaded scene with no delivered held keys.
        let mut reloaded_system = KeyboardInputSystem::new();
        let mut events = Events::default();
        reloaded_system.process_input(&held_repeat, false, &mut events);

        assert!(events.0.is_empty());
    }

    #[test]
    fn wheel_motion_is_a_scene_event_unless_text_entry_captures_it() {
        let mut scrolled = InputState::default();
        scrolled.wheel_delta = (0.0, -2.0);
        let mut system = KeyboardInputSystem::new();
        let mut events = Events::default();
        system.process_input(&InputState::default(), false, &mut events);
        system.process_input(&scrolled, true, &mut events);
        assert!(events.0.is_empty());

        system.process_input(&scrolled, false, &mut events);
        assert!(matches!(
            events.0.as_slice(),
            [EventSignal::MouseWheel { delta_x, delta_y }] if *delta_x == 0.0 && *delta_y == -2.0
        ));
    }

    #[test]
    fn right_drag_is_a_mouse_look_event_unless_text_entry_captures_it() {
        use crate::engine::user_input::MouseButton;
        let mut dragged = InputState::default();
        dragged.cursor_pos = Some((10.0, 10.0));
        dragged.start_frame();
        dragged.cursor_pos = Some((15.0, 7.0));
        dragged.mouse_down.insert(MouseButton::Right);
        dragged.start_frame();
        let mut system = KeyboardInputSystem::new();
        let mut events = Events::default();
        system.process_input(&dragged, true, &mut events);
        assert!(events.0.is_empty());

        system.process_input(&dragged, false, &mut events);
        assert!(matches!(
            events.0.as_slice(),
            [EventSignal::MouseLook { delta_x, delta_y }] if *delta_x == 5.0 && *delta_y == -3.0
        ));

        // Moving with only the left button held is not a look.
        let mut left = dragged.clone();
        left.mouse_down.clear();
        left.mouse_down.insert(MouseButton::Left);
        let mut events = Events::default();
        system.process_input(&left, false, &mut events);
        assert!(!events.0.iter().any(|event| matches!(event, EventSignal::MouseLook { .. })));
    }

    #[test]
    fn mouse_buttons_are_scene_events_named_by_button_unless_text_entry_captures_them() {
        use crate::engine::user_input::MouseButton;
        let mut pressed = InputState::default();
        pressed.mouse_pressed.insert(MouseButton::Middle);
        let mut system = KeyboardInputSystem::new();
        let mut events = Events::default();
        system.process_input(&pressed, true, &mut events);
        assert!(events.0.is_empty());

        system.process_input(&pressed, false, &mut events);
        assert!(matches!(
            events.0.as_slice(),
            [EventSignal::MouseButtonDown { button }] if button == "middle"
        ));

        let mut released = InputState::default();
        released.mouse_released.insert(MouseButton::Left);
        let mut events = Events::default();
        system.process_input(&released, false, &mut events);
        assert!(matches!(
            events.0.as_slice(),
            [EventSignal::MouseButtonUp { button }] if button == "left"
        ));
    }

    #[test]
    fn mouse_button_pressed_and_released_in_one_frame_follows_mouse_down() {
        use crate::engine::user_input::MouseButton;
        let mut system = KeyboardInputSystem::new();

        // Released, then pressed again before the frame ended: still held.
        let mut repressed = InputState::default();
        repressed.mouse_pressed.insert(MouseButton::Left);
        repressed.mouse_released.insert(MouseButton::Left);
        repressed.mouse_down.insert(MouseButton::Left);
        let mut events = Events::default();
        system.process_input(&repressed, false, &mut events);
        assert!(matches!(
            events.0.as_slice(),
            [EventSignal::MouseButtonUp { .. }, EventSignal::MouseButtonDown { .. }]
        ));

        // Pressed and released inside one frame: a click, no longer held.
        let mut clicked = repressed.clone();
        clicked.mouse_down.clear();
        let mut events = Events::default();
        system.process_input(&clicked, false, &mut events);
        assert!(matches!(
            events.0.as_slice(),
            [EventSignal::MouseButtonDown { .. }, EventSignal::MouseButtonUp { .. }]
        ));
    }
}
