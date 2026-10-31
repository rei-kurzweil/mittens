use super::system_world::SystemWorld;
use crate::engine::ecs::{CommandQueue, World};
use crate::engine::graphics::{RenderAssets, VisualWorld};

impl SystemWorld {
    /// Prepare the avatar capsule before resolving static contact.
    pub(super) fn prepare_avatar_capsule_contact(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
    ) {
        // Binding/configuration edits need one evaluation before contact. Source
        // pose changes already reach ancestor samplers through normal propagation.
        let pending = self
            .avatar_control
            .pending_capsule_samples(world, &self.transform_operator);
        for sample in pending {
            self.transform_changed(world, visuals, sample);
        }
        self.avatar_control.gate_capsule_contact(
            world,
            &self.transform_operator,
            &mut self.static_contact,
        );
    }

    pub(super) fn tick_grip_motion(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        queue: &mut CommandQueue,
        dt: f32,
    ) {
        self.grabbable.tick(world, assets, &mut self.rx, dt);
        self.attachment.tick(world, &mut self.rx);
        let _ = self.process_signals(world, visuals, assets, queue, 100_000);
        queue.flush(world, self, visuals, assets);
        if self.grabbable.climbing_moved() {
            self.prepare_avatar_capsule_contact(world, visuals);
            for target in self.static_contact.tick_excluding(world, &self.attachment) {
                self.transform_changed(world, visuals, target);
            }
        }
    }
}
