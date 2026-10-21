//! Shared input snapshots, engine-owned key/button types, and host HID providers.

mod providers;
pub use providers::{HidProvider, HidProviders, InputProviderId};
mod keyboard;
pub use keyboard::{
    Key, KeyCode, NamedKey, NativeKey, NativeKeyCode, PhysicalKey, logical_key, physical_key_code,
};
#[cfg(feature = "desktop")]
mod winit_adapter;
#[cfg(feature = "desktop")]
pub use winit_adapter::WinitHidProvider;

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
}

/// Normalized host events consumed by the same state processor on every platform.
#[derive(Debug, Clone, PartialEq)]
pub enum HidEvent {
    Keyboard {
        physical_key: PhysicalKey,
        logical_key: Key,
        state: ButtonState,
        repeat: bool,
        is_synthetic: bool,
        text: Option<String>,
    },
    Focused(bool),
    MouseButton {
        state: ButtonState,
        button: MouseButton,
    },
    CursorMoved {
        position: (f32, f32),
    },
    MouseWheel {
        delta: (f32, f32),
    },
}

/// One ordered gameplay-keyboard transition captured at the platform boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardInputRecord {
    /// Identifies the source so two keyboards can hold the same physical key.
    pub provider_id: InputProviderId,
    pub physical_key: PhysicalKey,
    pub code: Option<String>,
    pub key: String,
    pub transition: KeyboardTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardTransition {
    Down,
    Press,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputFrameEvent {
    InsertText(String),
    Backspace,
    DeleteForward,
    MoveCaretLeft,
    MoveCaretRight,
}

/// Snapshot of user input.
///
/// This is intentionally minimal for now, but it already supports:
/// - current key/button state (`down`)
/// - per-frame transitions (`pressed`/`released`)
/// - cursor position and wheel delta
/// - mouse movement delta
#[derive(Default, Debug, Clone)]
pub struct InputState {
    pub keys_down: HashSet<Key>,
    pub keys_pressed: HashSet<Key>,
    pub keys_released: HashSet<Key>,

    pub mouse_down: HashSet<MouseButton>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,

    /// Cursor position in physical pixels of the host input surface.
    pub cursor_pos: Option<(f32, f32)>,

    /// Previous cursor position (updated at `begin_frame`).
    prev_cursor_pos: Option<(f32, f32)>,

    /// Mouse movement delta since last frame (current - previous).
    mouse_movement: (f32, f32),

    /// Derived mouse drag state (active when a button is held while the cursor moves).
    mouse_dragging: bool,
    mouse_drag_delta: (f32, f32),

    /// Accumulated wheel delta since last `begin_frame`.
    pub wheel_delta: (f32, f32),

    text_input_events: Vec<TextInputFrameEvent>,
    keyboard_events: Vec<KeyboardInputRecord>,
    physical_keys_down: Vec<(PhysicalKey, Option<String>, String, Key)>,
    // None preserves manually supplied snapshots. A provider-produced snapshot
    // explicitly lists live sources, including an empty list for idle hosts.
    active_providers: Option<HashSet<InputProviderId>>,
}

impl InputState {
    fn release_all_keys(&mut self) {
        for (physical_key, code, key, gameplay_key) in self.physical_keys_down.drain(..) {
            self.keyboard_events.push(KeyboardInputRecord {
                provider_id: InputProviderId::MANUAL,
                physical_key,
                code,
                key,
                transition: KeyboardTransition::Up,
            });
            self.keys_released.insert(gameplay_key);
        }
        self.keys_down.clear();
    }

    /// Called at the start of a render/update frame.
    ///
    /// Important: this does **not** clear edge-triggered sets (`pressed`/`released`).
    /// Those are cleared at `end_frame` so events delivered before `RedrawRequested`
    /// are still visible to systems during `Universe::update`.
    pub fn start_frame(&mut self) {
        // Update mouse movement delta.
        self.mouse_movement = match (self.cursor_pos, self.prev_cursor_pos) {
            (Some((cx, cy)), Some((px, py))) => (cx - px, cy - py),
            _ => (0.0, 0.0),
        };
        self.prev_cursor_pos = self.cursor_pos;

        // Derive drag state from buttons + movement.
        let any_button_down = !self.mouse_down.is_empty();
        let moved = self.mouse_movement.0 != 0.0 || self.mouse_movement.1 != 0.0;
        self.mouse_dragging = any_button_down && moved;
        self.mouse_drag_delta = if self.mouse_dragging {
            self.mouse_movement
        } else {
            (0.0, 0.0)
        };
    }

    /// Clears edge-triggered sets at the end of a frame.
    pub fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.wheel_delta = (0.0, 0.0);
        self.text_input_events.clear();
        self.keyboard_events.clear();
    }

    #[inline]
    pub fn key_down(&self, key: &Key) -> bool {
        self.keys_down.contains(key)
    }

    #[inline]
    pub fn key_pressed(&self, key: &Key) -> bool {
        self.keys_pressed.contains(key)
    }

    #[inline]
    pub fn key_released(&self, key: &Key) -> bool {
        self.keys_released.contains(key)
    }

    /// Returns the mouse movement delta (dx, dy) since the last frame.
    /// Returns (0, 0) if cursor position is not available.
    #[inline]
    pub fn mouse_movement(&self) -> (f32, f32) {
        self.mouse_movement
    }

    /// Whether the user is currently dragging the mouse (button held + cursor moved this frame).
    #[inline]
    pub fn mouse_dragging(&self) -> bool {
        self.mouse_dragging
    }

    /// Mouse drag delta (dx, dy) in pixels for this frame.
    #[inline]
    pub fn mouse_drag_delta(&self) -> (f32, f32) {
        self.mouse_drag_delta
    }

    /// Whether the given mouse button is currently dragging (that button is held + cursor moved
    /// this frame).
    #[inline]
    pub fn mouse_dragging_button(&self, button: MouseButton) -> bool {
        self.mouse_down.contains(&button)
            && (self.mouse_movement.0 != 0.0 || self.mouse_movement.1 != 0.0)
    }

    /// Mouse drag delta (dx, dy) in pixels for this frame, gated to the given button.
    #[inline]
    pub fn mouse_drag_delta_button(&self, button: MouseButton) -> (f32, f32) {
        if self.mouse_dragging_button(button) {
            self.mouse_movement
        } else {
            (0.0, 0.0)
        }
    }

    #[inline]
    pub fn text_input_events(&self) -> &[TextInputFrameEvent] {
        &self.text_input_events
    }

    #[inline]
    pub fn keyboard_events(&self) -> &[KeyboardInputRecord] {
        &self.keyboard_events
    }

    pub fn push_keyboard_event(&mut self, event: KeyboardInputRecord) {
        self.keyboard_events.push(event);
    }

    pub fn push_text_input_event(&mut self, event: TextInputFrameEvent) {
        self.text_input_events.push(event);
    }

    pub(crate) fn provider_is_active(&self, provider: InputProviderId) -> bool {
        self.active_providers
            .as_ref()
            .is_none_or(|active| active.contains(&provider))
    }
}

/// Stateful input event processor.
#[derive(Default, Debug, Clone)]
pub struct UserInput {
    state: InputState,
}

impl UserInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &InputState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    pub fn start_frame(&mut self) {
        self.state.start_frame();
    }

    pub fn end_frame(&mut self) {
        self.state.end_frame();
    }

    /// Apply one platform-independent event. Providers can reuse this processor.
    pub fn handle_event(&mut self, event: &HidEvent) {
        match event {
            HidEvent::Keyboard {
                physical_key,
                logical_key: logical,
                state,
                repeat,
                is_synthetic,
                text,
            } => {
                fn normalize_key(key: &Key) -> Key {
                    match key {
                        // Treat ASCII letters case-insensitively by storing the lowercase form.
                        // This makes WASD/QE work regardless of Shift state.
                        Key::Character(s) => {
                            if s.len() == 1 {
                                let c = s.chars().next().unwrap_or('\0');
                                if c.is_ascii_alphabetic() {
                                    return Key::Character(c.to_ascii_lowercase().to_string());
                                }
                            }
                            Key::Character(s.clone())
                        }
                        _ => key.clone(),
                    }
                }

                let key = normalize_key(logical);
                let public_event = (physical_key_code(*physical_key), logical_key(logical));
                match state {
                    ButtonState::Pressed => {
                        if *is_synthetic {
                            return;
                        }
                        let was_physically_down = self
                            .state
                            .physical_keys_down
                            .iter()
                            .any(|(held_key, ..)| held_key == physical_key);
                        if !was_physically_down && !*repeat {
                            self.state.physical_keys_down.push((
                                *physical_key,
                                public_event.0.clone(),
                                public_event.1.clone(),
                                key.clone(),
                            ));
                            self.state.keyboard_events.push(KeyboardInputRecord {
                                provider_id: InputProviderId::MANUAL,
                                physical_key: *physical_key,
                                code: public_event.0.clone(),
                                key: public_event.1.clone(),
                                transition: KeyboardTransition::Down,
                            });
                        }
                        if *repeat && !was_physically_down {
                            return;
                        }
                        self.state.keyboard_events.push(KeyboardInputRecord {
                            provider_id: InputProviderId::MANUAL,
                            physical_key: *physical_key,
                            code: public_event.0,
                            key: public_event.1,
                            transition: KeyboardTransition::Press,
                        });
                        let was_down = self.state.keys_down.contains(&key);
                        self.state.keys_down.insert(key.clone());
                        if !was_down {
                            self.state.keys_pressed.insert(key);
                        }
                        match logical {
                            Key::Named(NamedKey::Backspace) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::Backspace);
                            }
                            Key::Named(NamedKey::Delete) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::DeleteForward);
                            }
                            Key::Named(NamedKey::ArrowLeft) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::MoveCaretLeft);
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::MoveCaretRight);
                            }
                            _ => {}
                        }
                        if let Some(text) = text.as_ref() {
                            let filtered: String =
                                text.chars().filter(|c| !c.is_control()).collect();
                            if !filtered.is_empty() {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::InsertText(filtered));
                            }
                        }
                    }
                    ButtonState::Released => {
                        if let Some(index) = self
                            .state
                            .physical_keys_down
                            .iter()
                            .position(|(held_key, ..)| held_key == physical_key)
                        {
                            let (_, code, logical_key, gameplay_key) =
                                self.state.physical_keys_down.remove(index);
                            self.state.keyboard_events.push(KeyboardInputRecord {
                                provider_id: InputProviderId::MANUAL,
                                physical_key: *physical_key,
                                code,
                                key: logical_key,
                                transition: KeyboardTransition::Up,
                            });
                            self.state.keys_down.remove(&gameplay_key);
                            self.state.keys_released.insert(gameplay_key);
                        } else {
                            self.state.keys_down.remove(&key);
                            self.state.keys_released.insert(key);
                        }
                    }
                }
            }

            HidEvent::Focused(false) => {
                self.state.release_all_keys();
                self.state
                    .mouse_released
                    .extend(self.state.mouse_down.drain());
                self.state.cursor_pos = None;
            }

            HidEvent::MouseButton { state, button } => match state {
                ButtonState::Pressed => {
                    let was_down = self.state.mouse_down.contains(button);
                    self.state.mouse_down.insert(*button);
                    if !was_down {
                        self.state.mouse_pressed.insert(*button);
                    }
                }
                ButtonState::Released => {
                    self.state.mouse_down.remove(button);
                    self.state.mouse_released.insert(*button);
                }
            },

            HidEvent::CursorMoved { position } => {
                self.state.cursor_pos = Some(*position);
            }
            HidEvent::MouseWheel { delta } => {
                self.state.wheel_delta.0 += delta.0;
                self.state.wheel_delta.1 += delta.1;
            }

            HidEvent::Focused(true) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard(
        physical: KeyCode,
        logical: Key,
        state: ButtonState,
        repeat: bool,
        text: Option<&str>,
    ) -> HidEvent {
        HidEvent::Keyboard {
            physical_key: physical.into(),
            logical_key: logical,
            state,
            repeat,
            is_synthetic: false,
            text: text.map(str::to_owned),
        }
    }

    #[test]
    fn portable_events_preserve_physical_identity_modifiers_and_ordered_text() {
        let mut input = UserInput::new();
        input.handle_event(&keyboard(
            KeyCode::ShiftLeft,
            Key::Named(NamedKey::Shift),
            ButtonState::Pressed,
            false,
            None,
        ));
        input.handle_event(&keyboard(
            KeyCode::KeyW,
            Key::Character("W".into()),
            ButtonState::Pressed,
            false,
            Some("W"),
        ));
        input.handle_event(&keyboard(
            KeyCode::KeyW,
            Key::Character("W".into()),
            ButtonState::Pressed,
            true,
            Some("W"),
        ));
        assert!(input.state().key_down(&Key::Character("w".into())));
        assert!(input.state().key_down(&Key::Named(NamedKey::Shift)));
        // A changed logical value on release must still release the physical W,
        // leaving Shift held and preserving W's original public key spelling.
        input.handle_event(&keyboard(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Released,
            false,
            None,
        ));
        assert!(!input.state().key_down(&Key::Character("w".into())));
        assert!(input.state().key_down(&Key::Named(NamedKey::Shift)));
        let records = input.state().keyboard_events();
        assert_eq!(
            records
                .iter()
                .map(|record| record.transition)
                .collect::<Vec<_>>(),
            vec![
                KeyboardTransition::Down,
                KeyboardTransition::Press,
                KeyboardTransition::Down,
                KeyboardTransition::Press,
                KeyboardTransition::Press,
                KeyboardTransition::Up,
            ]
        );
        assert_eq!(records.last().unwrap().code.as_deref(), Some("KeyW"));
        assert_eq!(records.last().unwrap().key, "W");
        assert_eq!(
            input.state().text_input_events(),
            &[
                TextInputFrameEvent::InsertText("W".into()),
                TextInputFrameEvent::InsertText("W".into()),
            ]
        );
    }

    #[test]
    fn portable_focus_loss_clears_keys_buttons_and_prevents_repeat_reactivation() {
        let mut input = UserInput::new();
        input.handle_event(&keyboard(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
            false,
            None,
        ));
        input.handle_event(&HidEvent::CursorMoved {
            position: (10.0, 10.0),
        });
        input.handle_event(&HidEvent::MouseButton {
            state: ButtonState::Pressed,
            button: MouseButton::Right,
        });
        input.poll();
        input.handle_event(&HidEvent::CursorMoved {
            position: (14.0, 17.0),
        });
        input.handle_event(&HidEvent::MouseWheel { delta: (1.0, -2.0) });
        let snapshot = input.poll();
        assert_eq!(snapshot.mouse_drag_delta(), (4.0, 7.0));
        assert_eq!(snapshot.wheel_delta, (1.0, -2.0));
        input.handle_event(&HidEvent::Focused(false));
        let snapshot = input.poll();
        assert!(snapshot.keys_down.is_empty());
        assert!(snapshot.mouse_down.is_empty());
        assert!(snapshot.mouse_released.contains(&MouseButton::Right));
        assert_eq!(snapshot.cursor_pos, None);
        input.handle_event(&keyboard(
            KeyCode::KeyW,
            Key::Character("w".into()),
            ButtonState::Pressed,
            true,
            None,
        ));
        let snapshot = input.poll();
        assert!(snapshot.keyboard_events().is_empty());
        assert!(snapshot.keys_down.is_empty());
    }

    #[test]
    fn public_keyboard_names_preserve_physical_and_logical_meaning() {
        assert_eq!(
            physical_key_code(KeyCode::KeyW.into()).as_deref(),
            Some("KeyW")
        );
        assert_eq!(
            physical_key_code(KeyCode::ShiftLeft.into()).as_deref(),
            Some("ShiftLeft")
        );
        assert_eq!(logical_key(&Key::Character("W".into())), "W");
        assert_eq!(logical_key(&Key::Character("é".into())), "é");
        assert_eq!(logical_key(&Key::Named(NamedKey::ArrowUp)), "ArrowUp");
        assert_eq!(logical_key(&Key::Dead(None)), "Dead");
    }

    #[test]
    fn focus_loss_releases_held_keys_in_press_order() {
        let mut input = InputState::default();
        let w = Key::Character("w".into());
        input.keys_down.insert(w.clone());
        input.physical_keys_down.push((
            KeyCode::ShiftLeft.into(),
            Some("ShiftLeft".into()),
            "Shift".into(),
            Key::Named(NamedKey::Shift),
        ));
        input.physical_keys_down.push((
            KeyCode::KeyW.into(),
            Some("KeyW".into()),
            "W".into(),
            w.clone(),
        ));

        input.release_all_keys();

        assert!(input.keys_down.is_empty());
        assert!(input.keys_released.contains(&w));
        assert_eq!(input.keyboard_events[0].code.as_deref(), Some("ShiftLeft"));
        assert_eq!(input.keyboard_events[1].code.as_deref(), Some("KeyW"));
    }
}
