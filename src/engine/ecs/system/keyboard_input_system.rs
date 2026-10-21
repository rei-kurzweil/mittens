use winit::keyboard::PhysicalKey;

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{IntentSignal, SignalEmitter};
    use crate::engine::user_input::{KeyboardInputRecord, KeyboardTransition};
    use winit::keyboard::KeyCode;

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
}
