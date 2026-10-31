use super::SystemWorld;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::{EventSignal, World};
use crate::engine::graphics::{RenderAssets, RenderUploader, VisualWorld};

impl SystemWorld {
    /// Prepare render state before issuing a frame.
    ///
    /// This flushes any pending renderables by uploading meshes and inserting GPU-ready
    /// instances into `VisualWorld`.
    pub fn prepare_render(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
        queue: &mut crate::engine::ecs::CommandQueue,
    ) {
        let mut destruction = std::mem::take(&mut self.destroyable);
        destruction.prepare(self, world, visuals, render_assets, uploader);
        self.destroyable = destruction;

        // MMS callbacks are serviced after simulation tick. Publish their
        // coalesced material updates before this frame's draw preparation.
        self.material.tick(world, visuals);
        self.renderable.refresh_material_topology(world, visuals);
        // Ensure any imported assets are registered before renderables try to resolve meshes/textures.
        self.gltf
            .flush_imports(render_assets, &mut self.texture, uploader);

        self.implicit_surface.reconcile_and_build(
            world,
            visuals,
            render_assets,
            uploader,
            &mut self.mesh_bounds,
        );

        let collapse_roots = self.combine_mesh.reconcile_and_build(
            world,
            visuals,
            render_assets,
            uploader,
            &mut self.renderable,
            &mut self.mesh_bounds,
        );
        for source_root in collapse_roots {
            if let Some(old_parent) = world.parent_of(source_root) {
                queue.push_event(
                    source_root,
                    EventSignal::ParentChanged {
                        child: source_root,
                        old_parent: Some(old_parent),
                        new_parent: None,
                    },
                );
            }
            self.remove_subtree_immediate(world, visuals, source_root);
        }

        let flushed_renderables =
            self.renderable
                .flush_pending(world, visuals, render_assets, uploader, queue);
        if flushed_renderables {
            self.clipping.resync_after_renderable_flush(world, visuals);
            // Material-base capture requests need the just-published surface
            // handles before allocating this frame's per-view runtime textures.
            self.mirror.refresh_captures(world, visuals);
            for texture in self.mirror.take_pending_texture_registrations() {
                self.register_texture(world, visuals, texture);
            }
        }

        self.render_to_texture.reconcile_consumers(world);
        self.render_to_texture.flush_pending(visuals, uploader);

        // Must run after renderables are flushed so instance handles exist.
        self.texture.flush_pending(world, visuals, uploader);
    }
}
