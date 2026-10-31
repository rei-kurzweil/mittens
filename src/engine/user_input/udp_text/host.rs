//! Example-host integration: remote edits retain normal TextInput focus/editing.
use super::UdpTextInputProvider;
use crate::engine::{
    Universe,
    ecs::{ComponentId, IntentValue, SignalEmitter, component::TextInputComponent},
    user_input::InputState,
};
use std::{io, net::SocketAddr};
pub struct UdpTerminalHost {
    target: ComponentId,
}
impl UdpTerminalHost {
    #[cfg(test)]
    pub(super) fn for_target(target: ComponentId) -> Self {
        Self { target }
    }
    pub fn install(scene: &mut Universe, address: SocketAddr, selector: &str) -> io::Result<Self> {
        let target = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some(selector))
            .filter(|&id| {
                scene
                    .world
                    .get_component_by_id_as::<TextInputComponent>(id)
                    .is_some()
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "UDP terminal TextInput was not found",
                )
            })?;
        scene
            .input_providers
            .add(UdpTextInputProvider::bind(address)?);
        Ok(Self { target })
    }
    /// Call after polling providers and before update. A received edit explicitly
    /// targets the terminal even after a file-browser click cleared text focus.
    pub fn prepare(&self, scene: &mut Universe, input: &InputState) {
        if input.text_input_events().is_empty() {
            return;
        }
        if scene
            .world
            .get_component_by_id_as::<TextInputComponent>(self.target)
            .is_some_and(|input| input.focused)
        {
            return;
        }
        scene.command_queue.push_intent_now(
            self.target,
            IntentValue::TextInputSetFocus {
                component_id: self.target,
            },
        );
        scene.systems.process_commands(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut scene.command_queue,
        );
    }
}
