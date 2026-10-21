use crate::engine::ecs::SignalEmitter;
use crate::engine::startup_trace::{StartupCheckpoint, log_startup_progress};
use crate::engine::user_input::{HidProviders, InputState};
use crate::engine::{ecs, graphics};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;
use winit::window::Window;

pub struct Universe {
    pub world: ecs::World,
    pub command_queue: ecs::CommandQueue,
    pub systems: ecs::SystemWorld,

    pub visuals: graphics::VisualWorld,
    pub render_assets: graphics::RenderAssets,
    pub input_providers: HidProviders,

    repl: Option<crate::engine::repl::Repl>,
    repl_backend: Option<crate::engine::repl::ReplBackend>,
    meow_meow_repl: Option<crate::scripting::repl::MeowMeowRepl>,
    runtime_spec_session: Option<crate::scripting::runner::RuntimeSpecSession>,

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
            input_providers: HidProviders::new(),

            repl: None,
            repl_backend: None,
            meow_meow_repl: None,
            runtime_spec_session: None,
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
        if self.repl.is_none() && self.meow_meow_repl.is_none() {
            match crate::engine::repl::Repl::try_new() {
                Ok(repl) => {
                    self.repl = Some(repl);
                    self.repl_backend = Some(crate::engine::repl::ReplBackend::new());
                    println!("[REPL] Ready. Commands: ls, cd <name>, cd .., cd /, pwd, help");
                }
                Err(error) => eprintln!("[REPL] {error}"),
            }
        }
    }

    /// Enable the persistent MMS-language stdin REPL.
    pub fn enable_meow_meow_repl(&mut self) {
        if self.repl.is_some() || self.meow_meow_repl.is_some() {
            eprintln!("[mms] an stdin REPL is already active");
            return;
        }
        match crate::scripting::repl::MeowMeowRepl::new() {
            Ok(repl) => {
                self.meow_meow_repl = Some(repl);
                println!("[mms] Ready. Enter MMS expressions; help() lists capabilities.");
            }
            Err(error) => eprintln!("[mms] {error}"),
        }
    }

    fn mittens_host<'a>(
        &'a mut self,
        intents: &'a mut Vec<ecs::IntentValue>,
    ) -> crate::scripting::host::MittensHost<'a, 'a> {
        crate::scripting::host::MittensHost::runtime_lease(
            &mut self.world,
            &mut self.systems.rx,
            &mut self.render_assets,
            &mut self.command_queue,
            intents,
        )
        .with_graphics_settings(&mut self.systems.graphics_settings)
        .with_xr_settings(&mut self.systems.xr_settings)
    }

    /// Load and retain one MMS source file through the engine-owned scripting
    /// facade. Live ECS services are assembled only for the evaluation lease.
    pub fn load_mms_source_at_path(
        &mut self,
        source: &str,
        path: &str,
    ) -> Result<crate::scripting::runner::EvalOutput, String> {
        let previous_settings = self.systems.graphics_settings.clone();
        let previous_xr_settings = self.systems.xr_settings.clone();
        self.systems.graphics_settings.reset();
        self.systems.xr_settings.reset();
        let mut intents = Vec::new();
        let session = {
            let host = self.mittens_host(&mut intents);
            match crate::scripting::runner::RuntimeSpecSession::start_at_path_with_host(
                source, path, host,
            ) {
                Ok(session) => session,
                Err(error) => {
                    self.systems.graphics_settings = previous_settings;
                    self.systems.xr_settings = previous_xr_settings;
                    return Err(error);
                }
            }
        };

        if let Some(mut previous) = self.runtime_spec_session.take() {
            previous.close();
        }
        self.runtime_spec_session = Some(session);

        let output = crate::scripting::runner::EvalOutput {
            intents: intents.clone(),
            errors: Vec::new(),
        };
        for intent in intents {
            self.command_queue
                .push_intent_now(ecs::ComponentId::default(), intent);
        }
        self.drain_pending_signals();
        Ok(output)
    }

    /// Stop the retained MMS session and make all of its callback routes inert.
    pub fn close_mms_session(&mut self) {
        if let Some(mut session) = self.runtime_spec_session.take() {
            session.close();
        }
        self.systems.graphics_settings.reset();
        self.systems.xr_settings.reset();
        self.drain_pending_signals();
    }

    /// Explicitly retry OpenXR initialization after launching without a runtime.
    pub fn retry_xr_runtime(&mut self) -> Result<(), String> {
        self.systems.xr.retry_runtime()
    }

    pub fn configure_vr_perf(&mut self, config: crate::engine::vr_perf::VrPerfConfig) {
        self.systems.set_vr_perf_enabled(true);
        self.systems.xr.configure_vr_perf(config);
    }

    pub fn exit_requested(&self) -> bool {
        self.systems.xr.vr_perf_exit_requested()
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
            &mut self.render_assets,
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
                parent: parent,
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
                parent: parent,
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

        self.command_queue
            .push_intent_now(parent, ecs::IntentValue::RemoveChildren { parent: parent });
        self.drain_pending_signals();
        Ok(children)
    }

    /// Clone a prefab subtree rooted at `prefab_root` and attach the cloned instance under `parent`.
    ///
    /// The cloned instance receives fresh `ComponentId`s and fresh GUIDs.
    ///
    /// Note: this is a structural/data clone via component `encode`/`decode`. If any components
    /// contain references to other components (for example IK targets), those references are
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
                parent: parent,
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

    fn sync_repl(&mut self, service_meow_meow: bool) {
        // Always drain queued system-driven REPL commands so they don't grow unbounded
        // when REPL is disabled.
        let scripted = self.systems.take_repl_commands();

        if let (Some(repl), Some(backend)) = (&self.repl, self.repl_backend.as_mut()) {
            backend.exec_all(&self.world, repl.try_recv_all());
            backend.exec_all(&self.world, scripted);
        }
        if service_meow_meow && let Some(repl) = self.meow_meow_repl.as_mut() {
            repl.sync(
                &mut self.world,
                &mut self.systems.rx,
                &mut self.render_assets,
                &mut self.command_queue,
            );
        }
    }

    fn service_runtime_spec_callbacks(&mut self) {
        let Some(mut session) = self.runtime_spec_session.take() else {
            return;
        };
        let mut intents = Vec::new();
        let errors = {
            let host = self.mittens_host(&mut intents);
            session.service_callbacks_with_host(host)
        };
        self.runtime_spec_session = Some(session);

        for error in errors {
            eprintln!("[mms] callback error: {error}");
        }
        for intent in intents {
            self.command_queue
                .push_intent_now(ecs::ComponentId::default(), intent);
        }
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

        self.systems.xr.prepare_for_renderer_init(&self.world);

        self.apply_renderer_settings();

        let xr_required = self.systems.xr.required_vulkan_extensions();
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
            self.systems.xr.set_preferred_swapchain_format(fmt);
        }

        if let Some(gfx) = self.renderer.xr_vulkan_graphics() {
            self.systems.xr.set_vulkan_graphics(gfx);
        }

        Ok(())
    }

    fn apply_renderer_settings(&mut self) {
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
    }

    /// Initialize an XR-only renderer without a Winit event loop, window,
    /// Vulkan surface, or desktop swapchain. Requires an available XR runtime.
    pub fn init_renderer_for_xr(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_renderer_settings();
        self.systems.xr.initialize_runtime()?;
        let (instance, system) = self
            .systems
            .xr
            .vulkan_runtime()
            .ok_or("OpenXR runtime not initialized")?;
        self.renderer.init_xr_graphics(&instance, system)?;
        let graphics = self
            .renderer
            .xr_vulkan_graphics()
            .ok_or("XR graphics not initialized")?;
        self.systems.xr.set_vulkan_graphics(graphics);
        self.systems.xr.retry_runtime()?;
        let format = self
            .systems
            .xr
            .swapchain_format()
            .ok_or("OpenXR swapchain not initialized")?;
        self.renderer.init_xr_scene(format)?;
        Ok(())
    }

    /// Resize the renderer when the window is resized.
    pub fn resize_renderer(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.visuals
            .set_viewport([size.width as f32, size.height as f32]);
        self.renderer.resize(size);
    }

    /// Poll zero or more host HID providers, then update the engine. With no
    /// providers, desktop input is neutral; XR and simulation remain active.
    pub fn update_with_input_providers(&mut self, dt_sec: f32) {
        let input = self.input_providers.poll();
        self.update(dt_sec, &input);
    }

    /// Game/update step using an explicitly supplied snapshot. Hosts normally
    /// use `update_with_input_providers`; this also supports replay and tests.
    pub fn update(&mut self, dt_sec: f32, input: &InputState) {
        let vr_update_started = self.systems.vr_perf_enabled().then(Instant::now);
        self.sync_repl(true);
        // 1. Process input events (handled inside systems for now).
        // 2. Let systems call methods on components,
        //      for example, to update transforms or renderables, which
        //      will update VisualWorld can update draw_batches and give Renderer a snapshot
        let mut runtime_session = self.runtime_spec_session.take();
        self.systems.tick_with_runtime_session(
            &mut self.world,
            &mut self.visuals,
            &mut self.render_assets,
            input,
            &mut self.command_queue,
            runtime_session.as_mut(),
            dt_sec,
        );
        self.runtime_spec_session = runtime_session;

        // Systems dispatch Rx events during tick. Run queued MMS callbacks
        // before the command drain so their intents take effect this frame.
        self.service_runtime_spec_callbacks();

        // Process commands after tick so any commands queued during tick are processed in the same frame
        let vr_commands_started = self.systems.vr_perf_enabled().then(Instant::now);
        self.systems.process_commands(
            &mut self.world,
            &mut self.visuals,
            &mut self.render_assets,
            &mut self.command_queue,
        );
        if let Some(update_started) = vr_update_started {
            self.systems.set_vr_perf_update_timings(
                update_started.elapsed(),
                vr_commands_started
                    .map(|started| started.elapsed())
                    .unwrap_or_default(),
            );
        }
        log_startup_progress(StartupCheckpoint::FirstUpdateCompleted);

        // Editor systems may enqueue REPL navigation commands during this update.
        // Sync once more so the REPL reflects the just-applied world topology.
        self.sync_repl(false);
    }

    pub fn render(&mut self) {
        let vr_prepare_started = self.systems.vr_perf_enabled().then(Instant::now);
        // Prepare render (mesh uploads) - cast renderer to trait
        self.systems.prepare_render(
            &mut self.world,
            &mut self.visuals,
            &mut self.render_assets,
            &mut self.renderer as &mut dyn graphics::RenderUploader,
            &mut self.command_queue,
        );
        if let Some(started) = vr_prepare_started {
            self.systems.set_vr_perf_prepare_render(started.elapsed());
        }
        let vr_perf_pre_xr = self.systems.vr_perf_pre_xr();
        self.systems.xr.set_vr_perf_pre_xr(vr_perf_pre_xr);
        log_startup_progress(StartupCheckpoint::RenderPrepared);

        // Render XR (if enabled) before the window present.
        self.systems
            .xr
            .render_xr(&self.world, &mut self.visuals, &mut self.renderer);
        log_startup_progress(StartupCheckpoint::XrRenderCompleted);

        // XR-only startup has no window output. Windowed startup also skips
        // presentation when there is no active Camera3D/Camera2D.
        if self.renderer.has_window_output() && self.systems.camera.has_active_window_camera() {
            // TODO: rebuild inspector around component graph instead of entities.
            self.renderer
                .render_visual_world(&mut self.visuals)
                .expect("render failed");
        }
        log_startup_progress(StartupCheckpoint::WindowRenderCompleted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::EmissiveComponent;
    use crate::engine::ecs::{ComponentId, EventSignal, Signal};

    fn labelled(universe: &Universe, name: &str) -> ComponentId {
        universe
            .world
            .all_components()
            .find(|&id| universe.world.component_label(id) == Some(name))
            .unwrap()
    }

    fn pump_settings_callbacks(universe: &mut Universe) {
        for _ in 0..4 {
            universe.systems.rx.begin_frame();
            universe.drain_pending_signals();
            let mut session = universe.runtime_spec_session.take().unwrap();
            let mut intents = Vec::new();
            let errors = session.service_callbacks_with_host(universe.mittens_host(&mut intents));
            universe.runtime_spec_session = Some(session);
            assert!(errors.is_empty(), "{errors:?}");
            for intent in intents {
                universe
                    .command_queue
                    .push_intent_now(ComponentId::default(), intent);
            }
        }
        universe.drain_pending_signals();
    }

    fn click(universe: &mut Universe, target: ComponentId) {
        universe.command_queue.push_event(
            target,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: target,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        );
        pump_settings_callbacks(universe);
    }

    #[test]
    fn info_panel_string_and_component_titles_preserve_header_ownership_and_restore() {
        use crate::engine::ecs::component::{RenderableComponent, TextComponent};
        fn below(world: &ecs::World, root: ComponentId, child: ComponentId) -> bool {
            let mut current = world.parent_of(child);
            while let Some(id) = current {
                if id == root {
                    return true;
                }
                current = world.parent_of(id);
            }
            false
        }
        let mut universe = Universe::new(ecs::World::default());
        universe.load_mms_source_at_path(r#"
            import { info_panel } from "../assets/components/ui/info_panel.mms"
            import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
            info_panel({
                root_name = "string_panel" width_gu = 32.0 unit_scale = 0.08
                title = "Plain title" content = T { Text { "Body" } }
            })
            let header = T {
                name = "custom_header"
                Grabbable.parent()
                Raycastable.enabled()
                Style { display("flex") flex_direction("row") width(100%) height(3.5) }
                T { Text { name = "custom_caption" "Custom title" } }
                R.square() { name = "custom_header_surface" }
            }
            graphics_settings_panel({ root_name = "custom_panel" title = header })
        "#, "examples/graphics-settings.mms").unwrap();
        let string_panel = labelled(&universe, "string_panel");
        let text_wrapper = universe
            .world
            .find_component(string_panel, "#info_panel_title")
            .unwrap();
        assert!(universe.world.all_components().any(|id| {
            below(&universe.world, text_wrapper, id)
                && universe
                    .world
                    .get_component_by_id_as::<TextComponent>(id)
                    .is_some_and(|text| text.text == "Plain title")
        }));
        let panel = labelled(&universe, "custom_panel");
        let header = labelled(&universe, "custom_header");
        let caption = labelled(&universe, "custom_caption");
        assert!(
            universe
                .world
                .find_component(panel, "#info_panel_title")
                .is_none(),
            "a custom bar must not be nested in the default title label"
        );
        assert!(below(&universe.world, panel, header));
        assert!(below(&universe.world, header, caption));
        let header_hit = labelled(&universe, "custom_header_surface");
        assert!(
            crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
                &universe.world,
                header_hit
            )
            .is_none(),
            "panel controls must not become editor selection targets"
        );
        assert_eq!(
            crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit(
                &universe.world,
                header_hit
            ),
            Some(panel),
            "the custom header's parent grab targets the whole panel"
        );
        let slider = labelled(&universe, "graphics_mirrors_quality_slider");
        let slider_hit = universe
            .world
            .all_components()
            .find(|id| {
                below(&universe.world, slider, *id)
                    && universe
                        .world
                        .get_component_by_id_as::<RenderableComponent>(*id)
                        .is_some()
            })
            .unwrap();
        assert_eq!(
            crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit(
                &universe.world,
                slider_hit
            ),
            None,
            "header-only grab must not cover slider geometry"
        );
        let toggle = universe
            .world
            .find_component(header, "#accordion_toggle")
            .unwrap();
        // A grip grab reparents this root away from the factory's layout slot.
        // Restore listeners and selection exclusion must travel with it.
        let original_parent = universe.world.parent_of(panel);
        let held_parent = universe
            .world
            .add_component(crate::engine::ecs::component::TransformComponent::new());
        universe.world.set_parent(panel, Some(held_parent)).unwrap();
        assert!(
            crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
                &universe.world,
                header_hit
            )
            .is_none()
        );
        for _ in 0..2 {
            click(&mut universe, toggle);
            assert!(
                universe
                    .world
                    .find_component(panel, "#accordion_body")
                    .is_none()
            );
            click(&mut universe, toggle);
            assert!(
                universe
                    .world
                    .find_component(panel, "#accordion_body")
                    .is_some()
            );
            assert_eq!(labelled(&universe, "custom_header"), header);
            assert_eq!(labelled(&universe, "custom_caption"), caption);
        }
        universe.world.set_parent(panel, original_parent).unwrap();
        let content = labelled(&universe, "graphics_settings_content");
        let rows: Vec<_> = universe
            .world
            .children_of(content)
            .iter()
            .copied()
            .filter(|id| {
                universe
                    .world
                    .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                        *id,
                    )
                    .is_some()
            })
            .collect();
        assert_eq!(rows.len(), 5);
        assert_eq!(
            universe.world.component_label(rows[0]),
            Some("graphics_mirrors_heading")
        );
        assert_eq!(
            universe.world.component_label(rows[3]),
            Some("graphics_secondary_motion_heading")
        );
        for (row, label) in [
            (rows[1], "Toggle mirrors"),
            (rows[2], "Mirror resolution"),
            (rows[4], "Toggle secondary motion"),
        ] {
            assert!(
                universe
                    .world
                    .all_components()
                    .any(|id| below(&universe.world, row, id)
                        && universe
                            .world
                            .get_component_by_id_as::<TextComponent>(id)
                            .is_some_and(|text| text.text == label)),
                "missing row label {label}"
            );
        }
        for id in universe.world.all_components().filter(|id| {
            below(&universe.world, panel, *id)
                && universe
                    .world
                    .get_component_by_id_as::<RenderableComponent>(*id)
                    .is_some()
        }) {
            assert!(
                crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
                    &universe.world,
                    id
                )
                .is_none(),
                "restored graphics panel renderables must not be selectable"
            );
        }
    }

    #[test]
    fn graphics_settings_toggle_updates_backend_and_restores_authoritative_values() {
        use crate::engine::ecs::component::{SliderComponent, TextComponent, ToggleComponent};
        let mut universe = Universe::new(ecs::World::default());
        universe
            .systems
            .toggle
            .install_handlers(&mut universe.systems.rx);
        universe.load_mms_source_at_path(r#"
            import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"
            graphics_settings_panel({ root_name = "test_graphics_settings" })
        "#, "examples/graphics-settings.mms").unwrap();
        assert!(universe.systems.graphics_settings.mirrors_enabled());
        let control = labelled(&universe, "graphics_mirrors_toggle_control");
        click(&mut universe, control);
        assert!(!universe.systems.graphics_settings.mirrors_enabled());
        assert!(!universe.visuals.mirror_captures_enabled());
        let motion_control = labelled(&universe, "graphics_secondary_motion_toggle_control");
        click(&mut universe, motion_control);
        assert!(
            !universe
                .systems
                .graphics_settings
                .secondary_motion_enabled()
        );
        let slider = labelled(&universe, "graphics_mirrors_quality_slider");
        universe.command_queue.push_intent_now(
            slider,
            ecs::IntentValue::SliderSet {
                component_id: slider,
                value: 5.0,
                emit_changed: true,
            },
        );
        pump_settings_callbacks(&mut universe);
        assert_eq!(
            universe.systems.graphics_settings.mirror_quality_override(),
            Some(1440)
        );
        assert_eq!(universe.visuals.mirror_quality_override(), Some(1440));
        let panel = labelled(&universe, "test_graphics_settings");
        let chevron = labelled(&universe, "accordion_toggle");
        for _ in 0..2 {
            click(&mut universe, chevron);
            assert!(
                universe
                    .world
                    .find_component(panel, "#accordion_body")
                    .is_none()
            );
            click(&mut universe, chevron);
            assert!(
                universe
                    .world
                    .find_component(panel, "#accordion_body")
                    .is_some()
            );
            assert!(
                !universe
                    .world
                    .get_component_by_id_as::<ToggleComponent>(labelled(
                        &universe,
                        "graphics_mirrors_toggle"
                    ))
                    .unwrap()
                    .value()
            );
            assert_eq!(
                universe
                    .world
                    .get_component_by_id_as::<SliderComponent>(labelled(
                        &universe,
                        "graphics_mirrors_quality_slider"
                    ))
                    .unwrap()
                    .value(),
                5.0
            );
            assert_eq!(
                universe
                    .world
                    .get_component_by_id_as::<TextComponent>(labelled(
                        &universe,
                        "graphics_mirrors_quality_readout"
                    ))
                    .unwrap()
                    .text,
                "1440"
            );
            assert!(
                !universe
                    .world
                    .get_component_by_id_as::<ToggleComponent>(labelled(
                        &universe,
                        "graphics_secondary_motion_toggle"
                    ))
                    .unwrap()
                    .value()
            );
        }
        // External changes synchronize the native control without emitting a
        // ToggleChanged callback or becoming a second settings authority.
        universe
            .systems
            .graphics_settings
            .set_mirrors_enabled(true, &mut universe.command_queue);
        pump_settings_callbacks(&mut universe);
        assert!(universe.visuals.mirror_captures_enabled());
        assert!(
            universe
                .world
                .get_component_by_id_as::<ToggleComponent>(labelled(
                    &universe,
                    "graphics_mirrors_toggle"
                ))
                .unwrap()
                .value()
        );
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<TextComponent>(labelled(
                    &universe,
                    "graphics_mirrors_enabled_readout"
                ))
                .unwrap()
                .text,
            "On"
        );
        universe
            .systems
            .graphics_settings
            .set_mirror_quality(700, &mut universe.command_queue);
        universe
            .systems
            .graphics_settings
            .set_secondary_motion_enabled(true, &mut universe.command_queue);
        pump_settings_callbacks(&mut universe);
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<TextComponent>(labelled(
                    &universe,
                    "graphics_mirrors_quality_readout"
                ))
                .unwrap()
                .text,
            "700"
        );
        assert_eq!(
            universe.systems.graphics_settings.mirror_quality_override(),
            Some(700),
            "silent slider sync must not snap an external setting to a preset"
        );
        assert!(
            universe
                .world
                .get_component_by_id_as::<ToggleComponent>(labelled(
                    &universe,
                    "graphics_secondary_motion_toggle"
                ))
                .unwrap()
                .value()
        );
        // Settings changed while the body is absent must be read on restore.
        click(&mut universe, chevron);
        universe
            .systems
            .graphics_settings
            .reset_mirror_quality(&mut universe.command_queue);
        universe
            .systems
            .graphics_settings
            .set_secondary_motion_enabled(false, &mut universe.command_queue);
        pump_settings_callbacks(&mut universe);
        click(&mut universe, chevron);
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<TextComponent>(labelled(
                    &universe,
                    "graphics_mirrors_quality_readout"
                ))
                .unwrap()
                .text,
            "Authored"
        );
        assert!(
            !universe
                .world
                .get_component_by_id_as::<ToggleComponent>(labelled(
                    &universe,
                    "graphics_secondary_motion_toggle"
                ))
                .unwrap()
                .value()
        );
    }

    #[test]
    fn xr_settings_host_validates_readback_and_resets_with_session() {
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                r#"
            XrSettings.floor.set_correction_meters(0.15)
            Text { name = "xr_readback" "" + XrSettings.floor.correction_meters() }
        "#,
                "examples/xr-settings.mms",
            )
            .unwrap();
        assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
        for source in [
            "XrSettings.floor.set_correction_meters(true)",
            "XrSettings.floor.set_correction_meters()",
            "XrSettings.floor.set_correction_meters(0.1, 0.2)",
            "XrSettings.floor.set_correction_meters(0.51)",
            "XrSettings.floor.set_correction_meters(-0.51)",
            "XrSettings.floor.correction_meters(0.1)",
        ] {
            assert!(
                universe
                    .load_mms_source_at_path(source, "examples/xr-settings.mms")
                    .is_err(),
                "{source}"
            );
            assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
        }
        let text = universe
            .world
            .get_component_by_id_as::<ecs::component::TextComponent>(labelled(
                &universe,
                "xr_readback",
            ))
            .unwrap();
        assert!((text.text.parse::<f32>().unwrap() - 0.15).abs() < 1e-6);
        universe
            .load_mms_source_at_path("Transform {}", "examples/xr-settings.mms")
            .unwrap();
        assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
        universe
            .systems
            .xr_settings
            .set_floor_correction_meters(-0.5, &mut universe.command_queue)
            .unwrap();
        universe.close_mms_session();
        assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
    }

    #[test]
    fn xr_settings_panel_slider_reset_external_updates_and_restore() {
        use ecs::component::{SliderComponent, TextComponent};
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                r##"
            import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
            T { name = "settings_mount" }
            let mount = query("#settings_mount")
            mount.attach(xr_settings_panel({ root_name = "test_xr_settings" }))
        "##,
                "examples/xr-settings.mms",
            )
            .unwrap();
        let slider = labelled(&universe, "xr_floor_correction_slider");
        universe.command_queue.push_intent_now(
            slider,
            ecs::IntentValue::SliderSet {
                component_id: slider,
                value: 0.15,
                emit_changed: true,
            },
        );
        pump_settings_callbacks(&mut universe);
        assert!((universe.systems.xr_settings.floor_correction_meters() - 0.15).abs() < 1e-6);
        let readout = |universe: &Universe| {
            universe
                .world
                .get_component_by_id_as::<TextComponent>(labelled(
                    universe,
                    "xr_floor_correction_readout",
                ))
                .unwrap()
                .text
                .clone()
        };
        assert_eq!(readout(&universe), "+15.0 cm");
        let panel = labelled(&universe, "test_xr_settings");
        let chevron = labelled(&universe, "accordion_toggle");
        for correction in [-0.2, 0.5, -0.5] {
            click(&mut universe, chevron);
            assert!(
                universe
                    .world
                    .find_component(panel, "#accordion_body")
                    .is_none()
            );
            universe
                .systems
                .xr_settings
                .set_floor_correction_meters(correction, &mut universe.command_queue)
                .unwrap();
            pump_settings_callbacks(&mut universe);
            click(&mut universe, chevron);
            let slider = labelled(&universe, "xr_floor_correction_slider");
            assert!(
                (universe
                    .world
                    .get_component_by_id_as::<SliderComponent>(slider)
                    .unwrap()
                    .value()
                    - correction as f32)
                    .abs()
                    < 1e-6
            );
        }
        universe
            .systems
            .xr_settings
            .set_floor_correction_meters(-0.125, &mut universe.command_queue)
            .unwrap();
        pump_settings_callbacks(&mut universe);
        assert_eq!(readout(&universe), "-12.5 cm");
        let reset = labelled(&universe, "xr_floor_correction_reset");
        click(&mut universe, reset);
        assert_eq!(universe.systems.xr_settings.floor_correction_meters(), 0.0);
        assert_eq!(readout(&universe), "0.0 cm");
    }

    #[test]
    fn settings_panels_in_grabbable_wrappers_allow_xr_slider_drags() {
        use ecs::component::{
            ControllerHand, ControllerPoseKind, ControllerXRComponent, PointerComponent,
            SliderComponent, TextComponent, TransformComponent,
        };
        use ecs::system::TransformSystem;
        use ecs::system::grabbable_system::grabbable_owner_for_hit;
        use ecs::system::pointer_system::PointerActivations;

        fn pointer_frame(
            universe: &mut Universe,
            driver: ComponentId,
            origin: [f32; 3],
            activations: PointerActivations,
        ) {
            universe
                .world
                .get_component_by_id_as_mut::<TransformComponent>(driver)
                .unwrap()
                .set_position(&mut universe.command_queue, origin[0], origin[1], origin[2]);
            universe.drain_pending_signals();
            universe.systems.rx.begin_frame();
            universe.systems.gesture.begin_frame();
            universe.systems.bvh.flush_pending(&universe.world);
            universe.systems.raycast.tick_with_queue(
                &mut universe.world,
                &mut universe.visuals,
                &InputState::default(),
                &mut universe.systems.rx,
                &universe.systems.bvh,
                &activations,
                &universe.systems.pointer,
                1.0 / 90.0,
            );
            universe.drain_pending_signals();
            universe.systems.gesture.tick_with_rx(
                &mut universe.world,
                &InputState::default(),
                &activations,
                &universe.systems.pointer,
                &universe.systems.raycast,
                &mut universe.systems.rx,
            );
            pump_settings_callbacks(universe);
        }

        for (factory, slider_name, readout_name, expected_text) in [
            (
                "graphics_settings_panel",
                "graphics_mirrors_quality_slider",
                "graphics_mirrors_quality_readout",
                "1440",
            ),
            (
                "xr_settings_panel",
                "xr_floor_correction_slider",
                "xr_floor_correction_readout",
                "+30.0 cm",
            ),
        ] {
            let mut universe = Universe::new(ecs::World::default());
            let source = format!(
                r#"
                import {{ {factory} }} from "../assets/components/panels/{factory}.mms"
                T.position(-2.6, 2.3, -1.5) {{
                    name = "settings_mount"
                    Grabbable {{}}
                    {factory}({{ root_name = "settings_panel" }})
                }}
                "#
            );
            universe
                .load_mms_source_at_path(&source, "examples/_settings_drag_test.mms")
                .unwrap();
            universe
                .systems
                .slider
                .install_handlers(&mut universe.systems.rx);
            universe
                .systems
                .gesture
                .install_handlers(&mut universe.systems.rx);
            let controller = universe.world.add_component(ControllerXRComponent::new(
                true,
                ControllerHand::Left,
                ControllerPoseKind::Aim,
            ));
            let driver = universe.world.add_component(TransformComponent::new());
            let pointer = universe.world.add_component(PointerComponent::new());
            universe.world.add_child(controller, driver).unwrap();
            universe.world.add_child(driver, pointer).unwrap();
            universe
                .world
                .init_component_tree(driver, &mut universe.command_queue);
            universe.drain_pending_signals();
            let panel = labelled(&universe, "settings_panel");
            let mount = labelled(&universe, "settings_mount");
            let panel_model = TransformSystem::world_model(&universe.world, panel).unwrap();

            for restored in [false, true] {
                if restored {
                    let toggle = labelled(&universe, "accordion_toggle");
                    click(&mut universe, toggle);
                    click(&mut universe, toggle);
                }
                universe
                    .systems
                    .layout
                    .tick(&mut universe.world, &mut universe.command_queue);
                universe.drain_pending_signals();
                let slider = labelled(&universe, slider_name);
                let config = universe
                    .world
                    .get_component_by_id_as::<SliderComponent>(slider)
                    .unwrap();
                let width = config.width();
                let track = universe
                    .world
                    .find_component(config.track_mount.unwrap(), "Renderable")
                    .unwrap();
                let slider_model = TransformSystem::world_model(&universe.world, slider).unwrap();
                let ray_origin = |fraction: f32| {
                    let point = crate::utils::math::mat4_mul_vec4(
                        slider_model,
                        [(fraction - 0.5) * width, 0.0, 0.0, 1.0],
                    );
                    [point[0], point[1], point[2] + 1.0]
                };
                pointer_frame(
                    &mut universe,
                    driver,
                    ray_origin(0.2),
                    PointerActivations {
                        pressed: vec![pointer],
                        down: vec![pointer],
                        ..Default::default()
                    },
                );
                assert_eq!(
                    universe.systems.gesture.state().drag_renderable,
                    Some(track),
                    "{factory}: XR trigger must acquire the track (restored={restored})"
                );
                pointer_frame(
                    &mut universe,
                    driver,
                    ray_origin(0.8),
                    PointerActivations {
                        down: vec![pointer],
                        ..Default::default()
                    },
                );
                assert_eq!(
                    universe
                        .world
                        .get_component_by_id_as::<TextComponent>(labelled(&universe, readout_name))
                        .unwrap()
                        .text,
                    expected_text,
                    "{factory}: moving the controller must apply the slider value"
                );
                if factory == "xr_settings_panel" {
                    assert!(
                        (universe.systems.xr_settings.floor_correction_meters() - 0.3).abs() < 1e-5
                    );
                } else {
                    assert_eq!(
                        universe.systems.graphics_settings.mirror_quality_override(),
                        Some(1440)
                    );
                }
                pointer_frame(
                    &mut universe,
                    driver,
                    ray_origin(0.8),
                    PointerActivations {
                        released: vec![pointer],
                        ..Default::default()
                    },
                );
                assert!(!universe.systems.gesture.state().dragging);
                assert_eq!(
                    TransformSystem::world_model(&universe.world, panel).unwrap(),
                    panel_model,
                    "slider input must not move the panel"
                );
                let title = universe.world.find_component(panel, "#title_bar").unwrap();
                let title_hit = universe.world.find_component(title, "Renderable").unwrap();
                assert_eq!(
                    grabbable_owner_for_hit(&universe.world, title_hit),
                    Some(mount)
                );
            }
        }
    }

    #[test]
    fn xr_settings_multiple_panels_share_backend_after_one_is_removed() {
        use ecs::component::SliderComponent;
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                r#"
            import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"
            xr_settings_panel({ root_name = "xr_panel_a" })
            xr_settings_panel({ root_name = "xr_panel_b" })
        "#,
                "examples/xr-settings.mms",
            )
            .unwrap();
        let panel_a = labelled(&universe, "xr_panel_a");
        let panel_b = labelled(&universe, "xr_panel_b");
        let slider_a = universe
            .world
            .find_component(panel_a, "#xr_floor_correction_slider")
            .unwrap();
        universe.command_queue.push_intent_now(
            slider_a,
            ecs::IntentValue::SliderSet {
                component_id: slider_a,
                value: -0.15,
                emit_changed: true,
            },
        );
        pump_settings_callbacks(&mut universe);
        let slider_b = universe
            .world
            .find_component(panel_b, "#xr_floor_correction_slider")
            .unwrap();
        assert!(
            (universe
                .world
                .get_component_by_id_as::<SliderComponent>(slider_b)
                .unwrap()
                .value()
                + 0.15)
                .abs()
                < 1e-6
        );
        universe.command_queue.push_intent_now(
            panel_a,
            ecs::IntentValue::RemoveSubtree {
                component_id: panel_a,
            },
        );
        pump_settings_callbacks(&mut universe);
        assert!(universe.world.get_component_record(panel_a).is_none());
        assert!((universe.systems.xr_settings.floor_correction_meters() + 0.15).abs() < 1e-6);
        universe
            .systems
            .xr_settings
            .set_floor_correction_meters(0.25, &mut universe.command_queue)
            .unwrap();
        pump_settings_callbacks(&mut universe);
        assert!(
            (universe
                .world
                .get_component_by_id_as::<SliderComponent>(slider_b)
                .unwrap()
                .value()
                - 0.25)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn xr_settings_example_loads_matching_graphics_scene_fixture() {
        use ecs::component::{CameraXRComponent, GLTFComponent, MirrorComponent};
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                include_str!("../../examples/xr-settings.mms"),
                "examples/xr-settings.mms",
            )
            .unwrap();
        for name in [
            "xr_settings",
            "xr_settings_title_bar",
            "xr_settings_mirror",
            "studio_floor",
            "rei_mu_xr_driver",
            "xr_floor_correction_slider",
        ] {
            labelled(&universe, name);
        }
        assert!(
            !universe
                .world
                .all_components()
                .any(|id| universe.world.component_label(id) == Some("graphics_settings"))
        );
        assert!(universe.world.all_components().any(|id| {
            universe
                .world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some()
        }));
        assert!(universe.world.all_components().any(|id| {
            universe
                .world
                .get_component_by_id_as::<MirrorComponent>(id)
                .is_some()
        }));
        assert!(universe.world.all_components().any(|id| {
            universe
                .world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some_and(|gltf| gltf.uri == "assets/models/rei(mu).glb")
        }));
    }

    #[test]
    fn graphics_settings_host_reads_applied_state_and_resets_on_scene_replacement() {
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                r#"
            let mirrors = GraphicsSettings.mirrors
            mirrors.toggle(false)
            Text { name = "settings_readback" "" + mirrors.enabled() }
        "#,
                "examples/graphics-settings.mms",
            )
            .unwrap();
        assert!(!universe.systems.graphics_settings.mirrors_enabled());
        assert!(!universe.visuals.mirror_captures_enabled());
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(labelled(
                    &universe,
                    "settings_readback"
                ))
                .unwrap()
                .text,
            "false"
        );
        universe
            .load_mms_source_at_path("Transform {}", "examples/graphics-settings.mms")
            .unwrap();
        assert!(universe.systems.graphics_settings.mirrors_enabled());
        universe
            .systems
            .graphics_settings
            .set_mirrors_enabled(false, &mut universe.command_queue);
        universe.close_mms_session();
        assert!(universe.systems.graphics_settings.mirrors_enabled());
        assert!(universe.visuals.mirror_captures_enabled());
    }

    #[test]
    fn graphics_settings_calls_reject_invalid_arguments_without_edit_mode() {
        let mut universe = Universe::new(ecs::World::default());
        for source in [
            "GraphicsSettings.mirrors.toggle(1)",
            "GraphicsSettings.mirrors.toggle()",
            "GraphicsSettings.mirrors.toggle(false, true)",
            "GraphicsSettings.mirrors.enabled(false)",
            "GraphicsSettings.mirrors.quality(512.5)",
            "GraphicsSettings.mirrors.quality(true)",
            "GraphicsSettings.mirrors.quality(2147483648)",
            "GraphicsSettings.mirrors.quality()",
            "GraphicsSettings.mirrors.quality_override(512)",
            "GraphicsSettings.mirrors.reset_quality(512)",
            "GraphicsSettings.secondary_motion.toggle(0)",
            "GraphicsSettings.secondary_motion.toggle()",
            "GraphicsSettings.secondary_motion.enabled(false)",
        ] {
            assert!(
                universe
                    .load_mms_source_at_path(source, "examples/graphics-settings.mms")
                    .is_err(),
                "accepted {source}"
            );
            assert!(universe.systems.graphics_settings.mirrors_enabled());
        }
        universe
            .load_mms_source_at_path(
                "GraphicsSettings.mirrors.toggle(false)",
                "examples/graphics-settings.mms",
            )
            .unwrap();
        assert!(!universe.visuals.mirror_captures_enabled());
        assert!(!universe.world.all_components().any(|id| {
            universe
                .world
                .get_component_by_id_as::<ecs::component::EditorComponent>(id)
                .is_some()
        }));
    }

    #[test]
    fn graphics_settings_quality_and_motion_host_readback_clamp_reset_and_scene_defaults() {
        use crate::engine::ecs::component::TextComponent;
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                r#"
            let mirrors = GraphicsSettings.mirrors
            mirrors.quality(-10)
            Text { name = "minimum" "" + mirrors.quality_override() }
            mirrors.quality(10000)
            Text { name = "maximum" "" + mirrors.quality_override() }
            mirrors.reset_quality()
            if mirrors.quality_override() == null { Text { name = "reset" "Authored" } }
            mirrors.quality(1024)
            GraphicsSettings.secondary_motion.toggle(false)
            Text { name = "motion" "" + GraphicsSettings.secondary_motion.enabled() }
        "#,
                "examples/graphics-settings.mms",
            )
            .unwrap();
        for (name, expected) in [
            ("minimum", "64"),
            ("maximum", "2048"),
            ("reset", "Authored"),
            ("motion", "false"),
        ] {
            assert_eq!(
                universe
                    .world
                    .get_component_by_id_as::<TextComponent>(labelled(&universe, name))
                    .unwrap()
                    .text,
                expected
            );
        }
        assert_eq!(
            universe.systems.graphics_settings.mirror_quality_override(),
            Some(1024)
        );
        assert_eq!(universe.visuals.mirror_quality_override(), Some(1024));
        assert!(
            !universe
                .systems
                .graphics_settings
                .secondary_motion_enabled()
        );
        // A failed replacement restores the previous settings rather than defaults.
        assert!(
            universe
                .load_mms_source_at_path(
                    "GraphicsSettings.mirrors.quality(false)",
                    "examples/graphics-settings.mms"
                )
                .is_err()
        );
        assert_eq!(
            universe.systems.graphics_settings.mirror_quality_override(),
            Some(1024)
        );
        assert!(
            !universe
                .systems
                .graphics_settings
                .secondary_motion_enabled()
        );
        universe
            .load_mms_source_at_path("Transform {}", "examples/graphics-settings.mms")
            .unwrap();
        assert_eq!(
            universe.systems.graphics_settings.mirror_quality_override(),
            None
        );
        assert_eq!(universe.visuals.mirror_quality_override(), None);
        assert!(
            universe
                .systems
                .graphics_settings
                .secondary_motion_enabled()
        );
        universe
            .systems
            .graphics_settings
            .set_mirror_quality(2048, &mut universe.command_queue);
        universe
            .systems
            .graphics_settings
            .set_secondary_motion_enabled(false, &mut universe.command_queue);
        universe.close_mms_session();
        assert_eq!(
            universe.systems.graphics_settings.mirror_quality_override(),
            None
        );
        assert!(
            universe
                .systems
                .graphics_settings
                .secondary_motion_enabled()
        );
    }

    #[test]
    fn mms_facade_installs_replaces_and_closes_the_retained_session() {
        let mut universe = Universe::new(ecs::World::default());
        let source = r#"Transform { name = "universe-mms-session" }"#;

        let output = universe
            .load_mms_source_at_path(source, "Cargo.toml")
            .unwrap();
        assert!(output.errors.is_empty());
        assert!(universe.runtime_spec_session.is_some());
        assert!(
            universe
                .world
                .all_components()
                .any(|id| { universe.world.component_label(id) == Some("universe-mms-session") })
        );

        universe
            .load_mms_source_at_path(
                r#"Transform { name = "replacement-mms-session" }"#,
                "Cargo.toml",
            )
            .unwrap();
        assert!(universe.runtime_spec_session.is_some());

        universe.close_mms_session();
        assert!(universe.runtime_spec_session.is_none());
    }

    #[test]
    fn mms_facade_services_retained_callbacks_with_a_fresh_host_lease() {
        let mut universe = Universe::new(ecs::World::default());
        universe
            .load_mms_source_at_path(
                r#"
                    let glow = Emissive.on() { name = "facade-glow" intensity(1.0) }
                    let root = Transform { name = "facade-root" glow }
                    on(root, "Click", fn(event) { glow.set_intensity(2.0) })
                    root
                "#,
                "Cargo.toml",
            )
            .unwrap();
        let root = universe
            .world
            .all_components()
            .find(|&id| universe.world.component_label(id) == Some("facade-root"))
            .unwrap();
        let glow = universe
            .world
            .all_components()
            .find(|&id| universe.world.component_label(id) == Some("facade-glow"))
            .unwrap();

        universe.systems.rx.dispatch_event_handlers(
            &mut universe.world,
            &Signal::event(
                root,
                EventSignal::Click {
                    raycaster: ComponentId::default(),
                    renderable: root,
                    hit_point: [0.0; 3],
                    screen_pos_px: None,
                },
            ),
        );
        universe.service_runtime_spec_callbacks();
        assert_eq!(
            universe
                .world
                .get_component_by_id_as::<EmissiveComponent>(glow)
                .unwrap()
                .intensity,
            2.0
        );
    }
}
