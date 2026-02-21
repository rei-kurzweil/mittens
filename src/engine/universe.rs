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

    /// Add a component subtree to the "live" universe by initializing it.
    ///
    /// This runs `Component::init` for `root` and any not-yet-initialized descendants.
    pub fn add(&mut self, root: ecs::ComponentId) {
        self.world
            .init_component_tree(root, &mut self.command_queue);
    }

    /// Add a signal handler rooted at `scope_root`.
    ///
    /// The handler runs when a matching signal occurs within the `scope_root` subtree.
    ///
    /// Note: handlers are function pointers (not closures) for now.
    pub fn add_signal_handler(
        &mut self,
        kind: ecs::SignalKind,
        scope_root: ecs::ComponentId,
        handler: ecs::SignalHandler,
    ) {
        self.systems.rx.add_handler(kind, scope_root, handler);
    }

    pub fn remove_signal_handler(
        &mut self,
        kind: ecs::SignalKind,
        scope_root: ecs::ComponentId,
        handler: ecs::SignalHandler,
    ) -> bool {
        self.systems.rx.remove_handler(kind, scope_root, handler)
    }


    /// Attach `child` under `parent`.
    ///
    /// If `parent` is already initialized, the newly-attached subtree rooted at `child`
    /// is initialized automatically.
    pub fn attach(
        &mut self,
        parent: ecs::ComponentId,
        child: ecs::ComponentId,
    ) -> Result<(), &'static str> {
        let old_parent = self.world.parent_of(child);
        self.world.add_child(parent, child)?;

        self.systems.rx.push(
            child,
            ecs::EventSignal::ParentChanged {
                child,
                old_parent,
                new_parent: Some(parent),
            },
        );

        if self.world.is_initialized(parent) {
            self.world
                .init_component_tree(child, &mut self.command_queue);
        }
        Ok(())
    }

    /// Remove the child at `index` from `parent` by deleting its subtree.
    ///
    /// This preserves the `parent` component and only deletes the selected child subtree.
    /// The deletion is applied when the command queue is processed.
    ///
    /// Returns the removed root child's `ComponentId`.
    pub fn remove_child(
        &mut self,
        parent: ecs::ComponentId,
        index: usize,
    ) -> Result<ecs::ComponentId, &'static str> {
        if self.world.get_component_record(parent).is_none() {
            return Err("parent does not exist");
        }
        let child = *self
            .world
            .children_of(parent)
            .get(index)
            .ok_or("child index out of range")?;

        // Detach immediately to avoid dangling parent->child edges until the queue flush.
        self.world.detach_from_parent(child);

        self.systems.rx.push(
            child,
            ecs::EventSignal::ParentChanged {
                child,
                old_parent: Some(parent),
                new_parent: None,
            },
        );

        self.command_queue.queue_remove_subtree(child);
        Ok(child)
    }

    /// Remove all children from `parent` by deleting each child subtree.
    ///
    /// This preserves the `parent` component and deletes each direct child and its descendants.
    /// The deletions are applied when the command queue is processed.
    ///
    /// Returns the removed child subtree root `ComponentId`s in the previous child order.
    pub fn remove_children(
        &mut self,
        parent: ecs::ComponentId,
    ) -> Result<Vec<ecs::ComponentId>, &'static str> {
        if self.world.get_component_record(parent).is_none() {
            return Err("parent does not exist");
        }

        // Snapshot child list because it mutates as we detach and queue deletions.
        let children: Vec<ecs::ComponentId> = self.world.children_of(parent).to_vec();
        for child in children.iter().copied() {
            self.world.detach_from_parent(child);

            self.systems.rx.push(
                child,
                ecs::EventSignal::ParentChanged {
                    child,
                    old_parent: Some(parent),
                    new_parent: None,
                },
            );

            self.command_queue.queue_remove_subtree(child);
        }

        Ok(children)
    }

    /// Clone a prefab subtree rooted at `prefab_root` and attach the cloned instance under `parent`.
    ///
    /// The cloned instance receives fresh `ComponentId`s and fresh GUIDs.
    ///
    /// Note: this is a structural/data clone via component `encode`/`decode`. If any components
    /// contain references to other components (e.g. Action targets/params), those references are
    /// currently copied as-is and may require a future fixup pass.
    pub fn attach_clone(
        &mut self,
        parent: ecs::ComponentId,
        prefab_root: ecs::ComponentId,
    ) -> Result<ecs::ComponentId, String> {
        let node = ecs::ComponentCodec::encode_subtree_node(&self.world, prefab_root)?;
        let new_root = ecs::ComponentCodec::decode_subtree_node_with_new_guids(
            &mut self.world,
            Some(parent),
            &node,
        )?;

        if self.world.get_component_record(new_root).is_none() {
            return Err("attach_clone: new root missing after decode".to_string());
        }

        if self.world.is_initialized(parent) {
            self.world
                .init_component_tree(new_root, &mut self.command_queue);
        }

        self.systems.rx.push(
            new_root,
            ecs::EventSignal::ParentChanged {
                child: new_root,
                old_parent: None,
                new_parent: Some(parent),
            },
        );

        Ok(new_root)
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

        self.renderer.init_for_window(
            window,
            xr_required
                .as_ref()
                .map(|(i, d)| (i.as_slice(), d.as_slice())),
        )?;

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
