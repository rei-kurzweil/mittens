//! Desktop-only translation of Winit events into engine HID events.
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};

impl From<winit::keyboard::NativeKeyCode> for NativeKeyCode {
    fn from(value: winit::keyboard::NativeKeyCode) -> Self {
        use winit::keyboard::NativeKeyCode as Native;
        match value {
            Native::Unidentified => Self::Unidentified,
            Native::Android(code) => Self::Android(code),
            Native::MacOS(code) => Self::MacOS(code),
            Native::Windows(code) => Self::Windows(code),
            Native::Xkb(code) => Self::Xkb(code),
        }
    }
}

impl From<winit::keyboard::PhysicalKey> for PhysicalKey {
    fn from(value: winit::keyboard::PhysicalKey) -> Self {
        match value {
            winit::keyboard::PhysicalKey::Code(code) => Self::Code(code.into()),
            winit::keyboard::PhysicalKey::Unidentified(code) => Self::Unidentified(code.into()),
        }
    }
}

impl From<&winit::keyboard::Key> for Key {
    fn from(value: &winit::keyboard::Key) -> Self {
        use winit::keyboard::{Key as PlatformKey, NativeKey as Native};
        match value {
            PlatformKey::Character(text) => Self::Character(text.to_string()),
            PlatformKey::Named(key) => Self::Named((*key).into()),
            PlatformKey::Dead(character) => Self::Dead(*character),
            PlatformKey::Unidentified(native) => Self::Unidentified(match native {
                Native::Unidentified => NativeKey::Unidentified,
                Native::Android(code) => NativeKey::Android(*code),
                Native::MacOS(code) => NativeKey::MacOS(*code),
                Native::Windows(code) => NativeKey::Windows(*code),
                Native::Xkb(code) => NativeKey::Xkb(*code),
                Native::Web(code) => NativeKey::Web(code.to_string()),
            }),
        }
    }
}

impl From<winit::event::MouseButton> for MouseButton {
    fn from(value: winit::event::MouseButton) -> Self {
        use winit::event::MouseButton as Button;
        match value {
            Button::Left => Self::Left,
            Button::Right => Self::Right,
            Button::Middle => Self::Middle,
            Button::Back => Self::Back,
            Button::Forward => Self::Forward,
            Button::Other(code) => Self::Other(code),
        }
    }
}

impl From<ElementState> for ButtonState {
    fn from(value: ElementState) -> Self {
        match value {
            ElementState::Pressed => Self::Pressed,
            ElementState::Released => Self::Released,
        }
    }
}

impl UserInput {
    /// Translate a window event, returning whether it was recognized as HID input.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        let event = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => HidEvent::Keyboard {
                physical_key: event.physical_key.into(),
                logical_key: (&event.logical_key).into(),
                state: event.state.into(),
                repeat: event.repeat,
                is_synthetic: *is_synthetic,
                text: event.text.as_ref().map(ToString::to_string),
            },
            WindowEvent::Focused(focused) => HidEvent::Focused(*focused),
            WindowEvent::MouseInput { state, button, .. } => HidEvent::MouseButton {
                state: (*state).into(),
                button: (*button).into(),
            },
            WindowEvent::CursorMoved { position, .. } => HidEvent::CursorMoved {
                position: (position.x as f32, position.y as f32),
            },
            WindowEvent::MouseWheel { delta, .. } => HidEvent::MouseWheel {
                delta: match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x as f32, position.y as f32)
                    }
                },
            },
            _ => return false,
        };
        self.handle_event(&event);
        true
    }
}

/// Shared handle: the window callback supplies events while the provider
/// collection polls the same adapter once per update.
#[derive(Debug, Clone)]
pub struct WinitHidProvider(Rc<RefCell<UserInput>>);

impl WinitHidProvider {
    pub fn new(input: UserInput) -> Self {
        Self(Rc::new(RefCell::new(input)))
    }

    pub fn handle_window_event(&self, event: &winit::event::WindowEvent) -> bool {
        self.0.borrow_mut().handle_window_event(event)
    }
}

impl HidProvider for WinitHidProvider {
    fn poll(&mut self) -> InputState {
        self.0.borrow_mut().poll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_keys_preserve_unicode_native_ids_and_public_aliases() {
        assert_eq!(
            Key::from(&winit::keyboard::Key::Character("é".into())),
            Key::Character("é".into())
        );
        assert_eq!(
            PhysicalKey::from(winit::keyboard::PhysicalKey::Unidentified(
                winit::keyboard::NativeKeyCode::Android(123)
            )),
            PhysicalKey::Unidentified(NativeKeyCode::Android(123))
        );
        assert_eq!(
            physical_key_code(PhysicalKey::Code(
                winit::keyboard::KeyCode::SuperLeft.into()
            )),
            Some("MetaLeft".into())
        );
        assert_eq!(
            logical_key(&Key::from(&winit::keyboard::Key::Named(
                winit::keyboard::NamedKey::Space
            ))),
            " "
        );
        assert_eq!(
            MouseButton::from(winit::event::MouseButton::Other(9)),
            MouseButton::Other(9)
        );
    }

    #[test]
    fn window_cursor_and_scroll_use_the_shared_state_processor() {
        let mut input = UserInput::new();
        input.handle_window_event(&WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(10.0, 12.0),
        });
        input.handle_window_event(&WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(1.0, 2.0),
            phase: winit::event::TouchPhase::Moved,
        });
        let snapshot = input.poll();
        assert_eq!(snapshot.cursor_pos, Some((10.0, 12.0)));
        assert_eq!(snapshot.wheel_delta, (1.0, 2.0));
        assert_eq!(input.poll().wheel_delta, (0.0, 0.0));
        input.handle_window_event(&WindowEvent::Focused(false));
        assert_eq!(input.poll().cursor_pos, None);
    }
}
