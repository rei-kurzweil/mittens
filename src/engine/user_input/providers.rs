use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{InputState, UserInput};

/// Stable source identity. Zero is reserved for snapshots
/// supplied directly to `Universe::update`, outside a provider collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputProviderId(u64);

impl InputProviderId {
    pub const MANUAL: Self = Self(0);
}

/// A host keyboard/pointer source. Each poll returns its current held state and
/// only the transitions/text/wheel events collected since its previous poll.
/// A provider that loses focus or disconnects must return neutral held state.
/// OpenXR pose/controller drivers operate independently of this HID collection.
pub trait HidProvider {
    fn poll(&mut self) -> InputState;
}

impl HidProvider for UserInput {
    fn poll(&mut self) -> InputState {
        self.start_frame();
        let snapshot = self.state().clone();
        self.end_frame();
        snapshot
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

struct ProviderEntry {
    id: InputProviderId,
    provider: Box<dyn HidProvider>,
    cursor_pos: Option<(f32, f32)>,
}

/// Zero or more HID sources combined into one frame snapshot for existing ECS
/// consumers. Providers are polled and their events delivered in insertion order.
pub struct HidProviders {
    providers: Vec<ProviderEntry>,
    previous: InputState,
    pointer_owner: Option<InputProviderId>,
    empty_logged: bool,
}

impl Default for HidProviders {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            previous: InputState::default(),
            pointer_owner: None,
            empty_logged: false,
        }
    }
}

impl fmt::Debug for HidProviders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HidProviders")
            .field("count", &self.len())
            .finish()
    }
}

impl HidProviders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, provider: impl HidProvider + 'static) -> InputProviderId {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = InputProviderId(
            NEXT_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .expect("HID provider ID space exhausted"),
        );
        self.providers.push(ProviderEntry {
            id,
            provider: Box::new(provider),
            cursor_pos: None,
        });
        id
    }

    pub fn remove(&mut self, id: InputProviderId) -> bool {
        let Some(index) = self.providers.iter().position(|entry| entry.id == id) else {
            return false;
        };
        self.providers.remove(index);
        true
    }

    pub fn clear(&mut self) {
        self.providers.clear();
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn poll(&mut self) -> InputState {
        if self.is_empty() {
            if !self.empty_logged {
                println!("[Input] No HID providers configured; keyboard/mouse input is idle.");
                self.empty_logged = true;
            }
        } else {
            self.empty_logged = false;
        }

        let active = self.providers.iter().map(|entry| entry.id).collect();
        let mut combined = InputState {
            active_providers: Some(active),
            ..Default::default()
        };
        let previous_pointer_owner = self.pointer_owner;
        if self
            .pointer_owner
            .is_some_and(|id| !combined.provider_is_active(id))
        {
            self.pointer_owner = None;
        }
        for entry in &mut self.providers {
            let mut snapshot = entry.provider.poll();
            combined.keys_down.extend(snapshot.keys_down.drain());
            combined.keys_pressed.extend(snapshot.keys_pressed.drain());
            combined
                .keys_released
                .extend(snapshot.keys_released.drain());
            combined
                .mouse_down
                .extend(snapshot.mouse_down.iter().copied());
            combined
                .mouse_pressed
                .extend(snapshot.mouse_pressed.iter().copied());
            combined
                .mouse_released
                .extend(snapshot.mouse_released.iter().copied());
            combined.wheel_delta.0 += snapshot.wheel_delta.0;
            combined.wheel_delta.1 += snapshot.wheel_delta.1;
            combined
                .text_input_events
                .append(&mut snapshot.text_input_events);
            for mut record in snapshot.keyboard_events.drain(..) {
                record.provider_id = entry.id;
                combined.keyboard_events.push(record);
            }

            // One shared pointer: the most recently active source wins. Idle
            // providers with cached coordinates must not overwrite its position.
            let pointer_active = snapshot.cursor_pos != entry.cursor_pos
                || !snapshot.mouse_pressed.is_empty()
                || !snapshot.mouse_released.is_empty()
                || snapshot.wheel_delta != (0.0, 0.0);
            if self.pointer_owner == Some(entry.id) && snapshot.cursor_pos.is_none() {
                self.pointer_owner = None;
                combined.cursor_pos = None;
            }
            if snapshot.cursor_pos.is_some() && (pointer_active || self.pointer_owner.is_none()) {
                self.pointer_owner = Some(entry.id);
            }
            if self.pointer_owner == Some(entry.id) {
                combined.cursor_pos = snapshot.cursor_pos;
            }
            entry.cursor_pos = snapshot.cursor_pos;
        }
        combined.keys_released.extend(
            self.previous
                .keys_down
                .difference(&combined.keys_down)
                .cloned(),
        );
        combined.mouse_released.extend(
            self.previous
                .mouse_down
                .difference(&combined.mouse_down)
                .copied(),
        );
        // Releasing one source must not release another source's held control.
        combined
            .keys_released
            .retain(|key| !combined.keys_down.contains(key));
        combined
            .mouse_released
            .retain(|button| !combined.mouse_down.contains(button));
        combined.prev_cursor_pos = if previous_pointer_owner == self.pointer_owner {
            self.previous.cursor_pos
        } else {
            // Changing pointer source must not create a jump/drag between devices.
            combined.cursor_pos
        };
        combined.start_frame();
        self.previous = combined.clone();
        combined
    }
}

#[cfg(test)]
mod tests;
