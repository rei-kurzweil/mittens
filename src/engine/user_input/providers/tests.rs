use std::collections::VecDeque;

use super::*;
use crate::engine::ecs::component::{InputComponent, TransformComponent};
use crate::engine::ecs::system::{InputSystem, KeyboardInputSystem};
use crate::engine::ecs::{ComponentId, EventSignal, IntentSignal, SignalEmitter, World};
use crate::engine::user_input::MouseButton;
use crate::engine::user_input::{Key, KeyCode, NamedKey, PhysicalKey};
use crate::engine::user_input::{KeyboardInputRecord, KeyboardTransition, TextInputFrameEvent};

struct Frames(VecDeque<InputState>);

impl Frames {
    fn new(frames: impl IntoIterator<Item = InputState>) -> Self {
        Self(frames.into_iter().collect())
    }
}

impl HidProvider for Frames {
    fn poll(&mut self) -> InputState {
        self.0.pop_front().expect("unexpected provider poll")
    }
}

#[derive(Default)]
struct Signals {
    events: Vec<EventSignal>,
    intents: Vec<IntentSignal>,
}

impl SignalEmitter for Signals {
    fn push_event(&mut self, _: ComponentId, event: EventSignal) {
        self.events.push(event);
    }
    fn push_intent(&mut self, _: ComponentId, intent: IntentSignal) {
        self.intents.push(intent);
    }
}

fn key_frame(
    key: Key,
    physical: KeyCode,
    down: bool,
    transitions: &[KeyboardTransition],
) -> InputState {
    let mut state = InputState::default();
    if down {
        state.keys_down.insert(key.clone());
    }
    for transition in transitions {
        match transition {
            KeyboardTransition::Down => {
                state.keys_pressed.insert(key.clone());
            }
            KeyboardTransition::Up => {
                state.keys_released.insert(key.clone());
            }
            _ => {}
        }
        state.push_keyboard_event(KeyboardInputRecord {
            provider_id: InputProviderId::MANUAL,
            physical_key: PhysicalKey::Code(physical),
            code: super::super::physical_key_code(physical.into()),
            key: super::super::logical_key(&key),
            transition: *transition,
        });
    }
    state
}

fn rig() -> (World, InputSystem, ComponentId) {
    let mut world = World::default();
    let input = world.add_component(InputComponent::new().with_speed(1.0));
    let transform = world.add_component(TransformComponent::new().with_position(1.0, 2.0, 3.0));
    world.add_child(input, transform).unwrap();
    let mut system = InputSystem::new();
    system.register_input(input);
    (world, system, transform)
}

#[test]
fn zero_providers_leave_transforms_untouched_and_emit_no_keyboard_events() {
    let mut providers = HidProviders::new();
    let (mut world, mut movement, transform) = rig();
    let before = world
        .get_component_by_id_as::<TransformComponent>(transform)
        .unwrap()
        .transform;
    let mut keyboard = KeyboardInputSystem::new();
    let mut signals = Signals::default();
    for _ in 0..3 {
        let snapshot = providers.poll();
        movement.process_input(&mut world, &snapshot, &mut signals, 1.0);
        keyboard.process_input(&snapshot, false, &mut signals);
        assert!(snapshot.keys_down.is_empty());
        assert!(snapshot.mouse_down.is_empty());
        assert!(snapshot.keyboard_events().is_empty());
        assert!(snapshot.text_input_events().is_empty());
        assert_eq!(snapshot.cursor_pos, None);
        assert_eq!(snapshot.mouse_movement(), (0.0, 0.0));
    }
    let after = world
        .get_component_by_id_as::<TransformComponent>(transform)
        .unwrap()
        .transform;
    assert_eq!(before.model, after.model);
    assert_eq!(before.rotation, after.rotation);
    assert_eq!(before.translation, after.translation);
    assert!(signals.events.is_empty());
    assert!(signals.intents.is_empty());
}

#[test]
fn one_provider_drives_movement_without_replaying_transitions() {
    let key = Key::Character("w".into());
    let mut providers = HidProviders::new();
    providers.add(Frames::new([
        key_frame(
            key.clone(),
            KeyCode::KeyW,
            true,
            &[KeyboardTransition::Down, KeyboardTransition::Press],
        ),
        key_frame(key, KeyCode::KeyW, true, &[]),
    ]));
    let (mut world, mut movement, transform) = rig();
    let mut keyboard = KeyboardInputSystem::new();
    let mut signals = Signals::default();
    for _ in 0..2 {
        let snapshot = providers.poll();
        movement.process_input(&mut world, &snapshot, &mut signals, 0.5);
        keyboard.process_input(&snapshot, false, &mut signals);
    }
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(transform)
            .unwrap()
            .transform
            .translation,
        [1.0, 3.0, 3.0]
    );
    assert_eq!(signals.events.len(), 2);
    assert!(matches!(&signals.events[0], EventSignal::KeyDown(_)));
    assert!(matches!(&signals.events[1], EventSignal::KeyPress(_)));
}

#[test]
fn identical_keys_and_buttons_from_two_sources_have_independent_lifetimes() {
    let key = Key::Character("w".into());
    let down = key_frame(
        key.clone(),
        KeyCode::KeyW,
        true,
        &[KeyboardTransition::Down, KeyboardTransition::Press],
    );
    let up = key_frame(key.clone(), KeyCode::KeyW, false, &[KeyboardTransition::Up]);
    let held = key_frame(key.clone(), KeyCode::KeyW, true, &[]);
    let with_button = |mut snapshot: InputState, held| {
        if held {
            snapshot.mouse_down.insert(MouseButton::Right);
        } else {
            snapshot.mouse_released.insert(MouseButton::Right);
        }
        snapshot
    };
    let mut providers = HidProviders::new();
    let first = providers.add(Frames::new([
        with_button(down.clone(), true),
        with_button(up.clone(), false),
        InputState::default(),
    ]));
    let second = providers.add(Frames::new([
        with_button(down, true),
        with_button(held, true),
        with_button(up, false),
    ]));
    let mut keyboard = KeyboardInputSystem::new();
    let mut signals = Signals::default();
    let snapshot = providers.poll();
    assert_eq!(snapshot.keyboard_events()[0].provider_id, first);
    assert_eq!(snapshot.keyboard_events()[2].provider_id, second);
    keyboard.process_input(&snapshot, false, &mut signals);
    assert_eq!(signals.events.len(), 4); // both physical sources delivered
    let snapshot = providers.poll();
    assert!(snapshot.key_down(&key));
    assert!(!snapshot.key_released(&key));
    assert!(snapshot.mouse_down.contains(&MouseButton::Right));
    assert!(!snapshot.mouse_released.contains(&MouseButton::Right));
    keyboard.process_input(&snapshot, false, &mut signals);
    let snapshot = providers.poll();
    assert!(!snapshot.key_down(&key));
    assert!(snapshot.key_released(&key));
    assert!(snapshot.mouse_released.contains(&MouseButton::Right));
    keyboard.process_input(&snapshot, false, &mut signals);
    assert_eq!(
        signals
            .events
            .iter()
            .filter(|event| matches!(event, EventSignal::KeyUp(_)))
            .count(),
        2
    );
}

#[test]
fn removing_the_last_provider_clears_latched_arrows_without_key_callbacks() {
    let mut providers = HidProviders::new();
    let id = providers.add(Frames::new([key_frame(
        Key::Named(NamedKey::ArrowLeft),
        KeyCode::ArrowLeft,
        true,
        &[KeyboardTransition::Down, KeyboardTransition::Press],
    )]));
    let (mut world, mut movement, transform) = rig();
    let mut keyboard = KeyboardInputSystem::new();
    let mut signals = Signals::default();
    let snapshot = providers.poll();
    movement.process_input(&mut world, &snapshot, &mut signals, 0.2);
    keyboard.process_input(&snapshot, false, &mut signals);
    let before = world
        .get_component_by_id_as::<TransformComponent>(transform)
        .unwrap()
        .transform;
    assert_ne!(before.rotation, [0.0, 0.0, 0.0, 1.0]);
    assert!(providers.remove(id));
    assert!(!providers.remove(id));
    signals.events.clear();
    signals.intents.clear();
    for _ in 0..3 {
        let snapshot = providers.poll();
        movement.process_input(&mut world, &snapshot, &mut signals, 0.2);
        // Capturing after removal must not fabricate releases for a dead source.
        keyboard.process_input(&snapshot, true, &mut signals);
        assert!(snapshot.keyboard_events().is_empty());
    }
    assert!(signals.events.is_empty());
    assert!(signals.intents.is_empty());
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(transform)
            .unwrap()
            .transform
            .model,
        before.model
    );
}

#[test]
fn pointer_ownership_is_stable_and_handoffs_do_not_create_drags() {
    let pointer = |x, right| {
        let mut snapshot = InputState {
            cursor_pos: Some((x, 0.0)),
            ..Default::default()
        };
        if right {
            snapshot.mouse_down.insert(MouseButton::Right);
        }
        snapshot
    };
    let mut providers = HidProviders::new();
    providers.add(Frames::new([
        pointer(0.0, true),
        pointer(5.0, true),
        pointer(5.0, true),
        pointer(5.0, true),
    ]));
    providers.add(Frames::new([
        pointer(100.0, false),
        pointer(100.0, false),
        pointer(100.0, false),
        pointer(110.0, false),
    ]));
    assert_eq!(providers.poll().mouse_movement(), (0.0, 0.0));
    let snapshot = providers.poll();
    assert_eq!(snapshot.cursor_pos, Some((5.0, 0.0)));
    assert_eq!(
        snapshot.mouse_drag_delta_button(MouseButton::Right),
        (0.0, 0.0)
    );
    let snapshot = providers.poll();
    assert_eq!(snapshot.cursor_pos, Some((5.0, 0.0))); // idle second source does not overwrite
    assert_eq!(snapshot.mouse_movement(), (0.0, 0.0));
    let snapshot = providers.poll();
    assert_eq!(snapshot.cursor_pos, Some((110.0, 0.0)));
    assert_eq!(snapshot.mouse_movement(), (0.0, 0.0));
}

#[test]
fn text_and_scroll_are_combined_in_provider_order_and_edges_are_consumed() {
    let mut first = UserInput::new();
    first.state_mut().wheel_delta = (1.0, 2.0);
    first
        .state_mut()
        .push_text_input_event(TextInputFrameEvent::InsertText("a".into()));
    let mut second = UserInput::new();
    second.state_mut().wheel_delta = (3.0, 4.0);
    second
        .state_mut()
        .push_text_input_event(TextInputFrameEvent::InsertText("b".into()));
    let mut providers = HidProviders::new();
    providers.add(first);
    providers.add(second);
    let snapshot = providers.poll();
    assert_eq!(snapshot.wheel_delta, (4.0, 6.0));
    assert_eq!(
        snapshot.text_input_events(),
        &[
            TextInputFrameEvent::InsertText("a".into()),
            TextInputFrameEvent::InsertText("b".into()),
        ]
    );
    let snapshot = providers.poll();
    assert!(snapshot.text_input_events().is_empty());
    assert_eq!(snapshot.wheel_delta, (0.0, 0.0));
}

#[test]
fn focus_loss_only_releases_the_affected_provider() {
    let mut first = UserInput::new();
    first
        .state_mut()
        .keys_down
        .insert(Key::Character("w".into()));
    first.state_mut().mouse_down.insert(MouseButton::Right);
    first.state_mut().cursor_pos = Some((20.0, 20.0));
    let handle = std::rc::Rc::new(std::cell::RefCell::new(first));
    let mut second = UserInput::new();
    second
        .state_mut()
        .keys_down
        .insert(Key::Character("w".into()));
    second.state_mut().mouse_down.insert(MouseButton::Right);
    let mut providers = HidProviders::new();
    providers.add(handle.clone());
    let second_id = providers.add(second);
    providers.poll();
    handle
        .borrow_mut()
        .handle_event(&crate::engine::user_input::HidEvent::Focused(false));
    let snapshot = providers.poll();
    assert!(snapshot.key_down(&Key::Character("w".into())));
    assert!(snapshot.mouse_down.contains(&MouseButton::Right));
    providers.remove(second_id);
    let snapshot = providers.poll();
    assert!(snapshot.keys_down.is_empty());
    assert!(snapshot.mouse_down.is_empty());
    assert!(snapshot.mouse_released.contains(&MouseButton::Right));
    assert_eq!(snapshot.cursor_pos, None);
    providers.clear();
    assert!(providers.is_empty());
}

#[test]
fn replacing_the_collection_does_not_suppress_a_new_sources_first_key() {
    let key = Key::Character("w".into());
    let down = key_frame(
        key,
        KeyCode::KeyW,
        true,
        &[KeyboardTransition::Down, KeyboardTransition::Press],
    );
    let mut providers = HidProviders::new();
    let first = providers.add(Frames::new([down.clone()]));
    let mut keyboard = KeyboardInputSystem::new();
    let mut signals = Signals::default();
    keyboard.process_input(&providers.poll(), false, &mut signals);
    providers = HidProviders::new();
    let second = providers.add(Frames::new([down]));
    assert_ne!(first, second);
    signals.events.clear();
    keyboard.process_input(&providers.poll(), false, &mut signals);
    assert_eq!(signals.events.len(), 2);
}
