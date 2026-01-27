use crate::engine::user_input::InputState;
use crate::engine::{ecs, graphics};
use std::sync::Arc;
use winit::window::Window;

pub struct Universe {
    pub world: ecs::World,
    pub command_queue: ecs::CommandQueue,
    pub systems: ecs::SystemWorld,

    pub visuals: graphics::VisualWorld,
    pub render_assets: graphics::RenderAssets,

    repl: Option<crate::engine::repl::Repl>,
    repl_backend: Option<crate::engine::repl::ReplBackend>,

    renderer: graphics::VulkanoRenderer,
}

impl Universe {
    pub fn new(world: ecs::World) -> Self {
        Self {
            world,
            command_queue: ecs::CommandQueue::new(),
            systems: ecs::SystemWorld::new(),

            visuals: graphics::VisualWorld::new(),
            render_assets: graphics::RenderAssets::new(),
            renderer: graphics::VulkanoRenderer::new(),

            repl: None,
            repl_backend: None,
        }
    }

    pub fn enable_repl(&mut self) {
        if self.repl.is_none() {
            self.repl = Some(crate::engine::repl::Repl::new());
            self.repl_backend = Some(crate::engine::repl::ReplBackend::new());
            println!("[REPL] Ready. Commands: ls, cd <name>, cd .., cd /, pwd, help");
        }
    }

    fn sync_repl(&mut self) {
        let (Some(repl), Some(backend)) = (&self.repl, self.repl_backend.as_mut()) else {
            return;
        };
        backend.exec_all(&self.world, repl.try_recv_all());
    }

    /// Initialize the renderer for a window.
    /// This must be called before rendering.
    pub fn init_renderer_for_window(
        &mut self,
        window: &Arc<Window>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let size = window.inner_size();
        self.visuals
            .set_viewport([size.width as f32, size.height as f32]);

        let xr_required = self.systems.openxr.required_vulkan_extensions();
        if let Some((ref instance_exts, ref device_exts)) = xr_required {
            println!(
                "[OpenXR] Required Vulkan extensions: instance={} device={}",
                instance_exts.len(),
                device_exts.len()
            );
            println!(
                "[OpenXR] Required Vulkan instance extensions: {}",
                instance_exts.join(" ")
            );
            println!(
                "[OpenXR] Required Vulkan device extensions: {}",
                device_exts.join(" ")
            );
        }

        self.renderer
            .init_for_window(window, xr_required.as_ref().map(|(i, d)| (i.as_slice(), d.as_slice())))?;

        if let Some(fmt) = self.renderer.window_vk_format_raw() {
            self.systems.openxr.set_preferred_swapchain_format(fmt);
        }

        if let Some(gfx) = self.renderer.xr_vulkan_graphics() {
            self.systems.openxr.set_vulkan_graphics(gfx);
        }

        Ok(())
    }

    /// Resize the renderer when the window is resized.
    pub fn resize_renderer(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.visuals
            .set_viewport([size.width as f32, size.height as f32]);
        self.renderer.resize(size);
    }

    /// Game/update step
    pub fn update(&mut self, dt_sec: f32, input: &InputState) {
        self.sync_repl();

        // 1. Process input events (handled inside systems for now).
        // 2. Let systems call methods on components,
        //      for example, to update transforms or renderables, which
        //      will update VisualWorld can update draw_batches and give Renderer a snapshot
        self.systems.tick(
            &mut self.world,
            &mut self.visuals,
            input,
            &mut self.command_queue,
            dt_sec,
        );

        // Process commands after tick so any commands queued during tick are processed in the same frame
        self.systems
            .process_commands(&mut self.world, &mut self.visuals, &mut self.command_queue);
    }

    pub fn render(&mut self) {
        // Prepare render (mesh uploads) - cast renderer to trait
        self.systems.prepare_render(
            &mut self.world,
            &mut self.visuals,
            &mut self.render_assets,
            &mut self.renderer as &mut dyn graphics::RenderUploader,
        );

        // Render XR (if enabled) before the window present.
        self.systems
            .openxr
            .render_xr(&self.world, &mut self.visuals, &mut self.renderer);

        // TODO: rebuild inspector around component graph instead of entities.

        self.renderer
            .render_visual_world(&mut self.visuals)
            .expect("render failed");
    }
}
