//! Input handling (winit -> engine state).
//!
//! Goal: keep `Windowing` focused on window lifecycle + rendering, while `UserInput`
//! owns interpreting window events into a small, reusable `InputState`.

use std::collections::HashSet;

use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};

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

    /// Cursor position in physical pixels (as reported by winit).
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
}

impl InputState {
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

    /// Feed a winit event into this input handler.
    ///
    /// Returns `true` if the event was recognized/consumed as input.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                fn normalize_key(key: &Key) -> Key {
                    match key {
                        // Treat ASCII letters case-insensitively by storing the lowercase form.
                        // This makes WASD/QE work regardless of Shift state.
                        Key::Character(s) => {
                            if s.len() == 1 {
                                let c = s.chars().next().unwrap_or('\0');
                                if c.is_ascii_alphabetic() {
                                    return Key::Character(
                                        c.to_ascii_lowercase().to_string().into(),
                                    );
                                }
                            }
                            Key::Character(s.clone())
                        }
                        _ => key.clone(),
                    }
                }

                let key = normalize_key(&event.logical_key);
                match event.state {
                    ElementState::Pressed => {
                        let was_down = self.state.keys_down.contains(&key);
                        self.state.keys_down.insert(key.clone());
                        if !was_down {
                            self.state.keys_pressed.insert(key);
                        }
                        match &event.logical_key {
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
                        if let Some(text) = event.text.as_ref() {
                            let filtered: String =
                                text.chars().filter(|c| !c.is_control()).collect();
                            if !filtered.is_empty() {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::InsertText(filtered));
                            }
                        }
                    }
                    ElementState::Released => {
                        self.state.keys_down.remove(&key);
                        self.state.keys_released.insert(key);
                    }
                }
                true
            }

            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    ElementState::Pressed => {
                        let was_down = self.state.mouse_down.contains(button);
                        self.state.mouse_down.insert(*button);
                        if !was_down {
                            self.state.mouse_pressed.insert(*button);
                        }
                    }
                    ElementState::Released => {
                        self.state.mouse_down.remove(button);
                        self.state.mouse_released.insert(*button);
                    }
                }
                true
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.state.cursor_pos = Some((position.x as f32, position.y as f32));
                true
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };
                self.state.wheel_delta.0 += dx;
                self.state.wheel_delta.1 += dy;
                true
            }

            _ => false,
        }
    }
}
