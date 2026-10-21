//! A consumer supplies HID input using only the public engine API. This target
//! also runs with --no-default-features, without a window/backend dependency.
use std::collections::VecDeque;

use mittens_engine::engine::user_input::{
    ButtonState, HidEvent, HidProvider, HidProviders, InputProviderId, InputState, Key, KeyCode,
    KeyboardTransition, UserInput,
};

struct ApplicationHidProvider {
    events: VecDeque<HidEvent>,
    input: UserInput,
}

impl HidProvider for ApplicationHidProvider {
    fn poll(&mut self) -> InputState {
        for event in self.events.drain(..) {
            self.input.handle_event(&event);
        }
        self.input.poll()
    }
}

#[test]
fn consumer_supplies_normalized_hid_without_a_platform_backend() {
    let application_provider = ApplicationHidProvider {
        events: VecDeque::from([HidEvent::Keyboard {
            physical_key: KeyCode::KeyW.into(),
            logical_key: Key::Character("W".into()),
            state: ButtonState::Pressed,
            repeat: false,
            is_synthetic: false,
            text: Some("W".into()),
        }]),
        input: UserInput::new(),
    };
    let mut providers = HidProviders::new();
    let id = providers.add(application_provider);
    assert_ne!(id, InputProviderId::MANUAL);
    let frame = providers.poll();
    assert!(frame.key_down(&Key::Character("w".into())));
    let records = frame.keyboard_events();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].provider_id, id);
    assert_eq!(records[0].code.as_deref(), Some("KeyW"));
    assert_eq!(records[0].key, "W");
    assert_eq!(records[0].transition, KeyboardTransition::Down);
    let next = providers.poll();
    assert!(next.key_down(&Key::Character("w".into())));
    assert!(next.keyboard_events().is_empty());
    providers.remove(id);
    let idle = providers.poll();
    assert!(idle.keys_down.is_empty());
    assert!(idle.keyboard_events().is_empty());
}
