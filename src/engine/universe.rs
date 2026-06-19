use crate::engine::ecs::SignalEmitter;
use crate::engine::startup_trace::{StartupCheckpoint, log_startup_progress};
use crate::engine::user_input::InputState;
use crate::engine::{ecs, graphics};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc;
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

    pub fn set_msaa_mode(&mut self, mode: graphics::MsaaMode) -> Result<(), &'static str> {
        self.renderer.set_msaa_mode(mode)
    }

    pub fn preferred_window_size(&self) -> Option<[u32; 2]> {
        self.visuals.preferred_window_size().or_else(|| {
            self.world.all_components().find_map(|cid| {
                self.world
                    .get_component_by_id_as::<ecs::component::RendererSettingsComponent>(cid)
                    .and_then(|s| s.window_size)
            })
        })
    }

    pub fn take_preferred_window_size(&mut self) -> Option<[u32; 2]> {
        self.visuals.take_preferred_window_size()
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

    fn drain_pending_signals(&mut self) {
        // Universe helpers are synchronous convenience APIs.
        // They emit intents and then drain them immediately so the caller sees the effect.
        self.systems.process_commands(
            &mut self.world,
            &mut self.visuals,
            &self.render_assets,
            &mut self.command_queue,
        );
    }

    // --- Query helpers (read-only World access) ---
    pub fn parent_of(&self, c: ecs::ComponentId) -> Option<ecs::ComponentId> {
        self.world.parent_of(c)
    }

    pub fn children_of(&self, c: ecs::ComponentId) -> &[ecs::ComponentId] {
        self.world.children_of(c)
    }

    pub fn get_component_by_id_as<T: 'static>(&self, c: ecs::ComponentId) -> Option<&T> {
        self.world.get_component_by_id_as::<T>(c)
    }

    pub fn component_name(&self, c: ecs::ComponentId) -> Option<&str> {
        self.world.component_name(c)
    }

    pub fn find_component(
        &mut self,
        root: ecs::ComponentId,
        selector: &str,
    ) -> Option<ecs::ComponentId> {
        let (tx, rx) = mpsc::channel();
        self.command_queue.push_intent_now(
            root,
            ecs::IntentValue::QueryFindComponent {
                root,
                selector: selector.to_string(),
                reply: tx,
            },
        );
        self.drain_pending_signals();
        rx.recv().ok().flatten()
    }

    pub fn find_all_components(
        &mut self,
        root: ecs::ComponentId,
        selector: &str,
    ) -> Vec<ecs::ComponentId> {
        let (tx, rx) = mpsc::channel();
        self.command_queue.push_intent_now(
            root,
            ecs::IntentValue::QueryFindAllComponents {
                root,
                selector: selector.to_string(),
                reply: tx,
            },
        );
        self.drain_pending_signals();
        rx.recv().unwrap_or_default()
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
        if self.world.get_component_record(parent).is_none() {
            return Err("parent does not exist");
        }
        if self.world.get_component_record(child).is_none() {
            return Err("child does not exist");
        }

        self.command_queue.push_intent_now(
            child,
            ecs::IntentValue::Attach {
                parents: vec![parent],
                child,
            },
        );
        self.drain_pending_signals();
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

        self.command_queue.push_intent_now(
            parent,
            ecs::IntentValue::RemoveChild {
                parents: vec![parent],
                index,
            },
        );
        self.drain_pending_signals();
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

        // Snapshot child list for the return value.
        let children: Vec<ecs::ComponentId> = self.world.children_of(parent).to_vec();

        self.command_queue.push_intent_now(
            parent,
            ecs::IntentValue::RemoveChildren {
                parents: vec![parent],
            },
        );
        self.drain_pending_signals();
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
        if self.world.get_component_record(parent).is_none() {
            return Err("attach_clone: parent does not exist".to_string());
        }
        if self.world.get_component_record(prefab_root).is_none() {
            return Err("attach_clone: prefab_root does not exist".to_string());
        }

        let before: HashSet<ecs::ComponentId> =
            self.world.children_of(parent).iter().copied().collect();

        self.command_queue.push_intent_now(
            parent,
            ecs::IntentValue::AttachClone {
                parents: vec![parent],
                prefab_root,
            },
        );
        self.drain_pending_signals();

        let after_children = self.world.children_of(parent);
        let mut new_children = after_children
            .iter()
            .copied()
            .filter(|c| !before.contains(c))
            .collect::<Vec<_>>();

        if new_children.len() != 1 {
            new_children.sort();
            return Err(format!(
                "attach_clone: expected exactly 1 new child under parent; got {} ({new_children:?})",
                new_children.len()
            ));
        }

        Ok(new_children[0])
    }

    fn sync_repl(&mut self) {
        // Always drain queued system-driven REPL commands so they don't grow unbounded
        // when REPL is disabled.
        let scripted = self.systems.take_repl_commands();

        let (Some(repl), Some(backend)) = (&self.repl, self.repl_backend.as_mut()) else {
            return;
        };

        backend.exec_all(&self.world, repl.try_recv_all());
        backend.exec_all(&self.world, scripted);
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

        // Apply renderer settings from the component graph if the caller didn't explicitly
        // override them (e.g. via CLI).
        if self.renderer.msaa_mode_override().is_none() {
            for cid in self.world.all_components() {
                if let Some(s) = self
                    .world
                    .get_component_by_id_as::<ecs::component::RendererSettingsComponent>(cid)
                {
                    let _ = self.renderer.set_msaa_mode(s.msaa_mode());
                }
            }
        }

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
            &self.render_assets,
            input,
            &mut self.command_queue,
            dt_sec,
        );

        // Process commands after tick so any commands queued during tick are processed in the same frame
        self.systems.process_commands(
            &mut self.world,
            &mut self.visuals,
            &self.render_assets,
            &mut self.command_queue,
        );
        log_startup_progress(StartupCheckpoint::FirstUpdateCompleted);

        // Editor systems may enqueue REPL navigation commands during this update.
        // Sync once more so the REPL reflects the just-applied world topology.
        self.sync_repl();
    }

    pub fn render(&mut self) {
        // Prepare render (mesh uploads) - cast renderer to trait
        self.systems.prepare_render(
            &mut self.world,
            &mut self.visuals,
            &mut self.render_assets,
            &mut self.renderer as &mut dyn graphics::RenderUploader,
            &mut self.command_queue,
        );
        log_startup_progress(StartupCheckpoint::RenderPrepared);

        // Render XR (if enabled) before the window present.
        self.systems
            .openxr
            .render_xr(&self.world, &mut self.visuals, &mut self.renderer);
        log_startup_progress(StartupCheckpoint::XrRenderCompleted);

        // Skip the window scene draw when there is no active Camera3D/Camera2D.
        // The winit loop and XR rendering continue to run independently.
        if self.systems.camera.has_active_window_camera() {
            // TODO: rebuild inspector around component graph instead of entities.
            self.renderer
                .render_visual_world(&mut self.visuals)
                .expect("render failed");
        }
        log_startup_progress(StartupCheckpoint::WindowRenderCompleted);
    }
}
