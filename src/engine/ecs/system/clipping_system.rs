use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::RenderableComponent;
use crate::engine::graphics::VisualWorld;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) const OWNED_LAYOUT_STENCIL_CLIP_LABEL: &str = "__layout_stencil_clip";

#[derive(Debug)]
struct PanelClipDebugInfo {
    label: String,
    scope_root: ComponentId,
    bg_id: ComponentId,
    stencil_clip_id: ComponentId,
    renderable_id: ComponentId,
    renderable_guid: uuid::Uuid,
}

#[derive(Debug, Default)]
pub struct ClippingSystem {
    active_stencil_clips: HashSet<ComponentId>,
}

impl ClippingSystem {
    pub fn register_renderable(
        &self,
        world: &World,
        visuals: &mut VisualWorld,
        renderable_component: ComponentId,
    ) {
        self.sync_renderable_stencil_ref(world, visuals, renderable_component);
    }

    pub fn register_stencil_clip(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        repl_command_queue: &mut Vec<String>,
    ) {
        self.active_stencil_clips.insert(component);

        let stencil_ref = Self::stencil_ref_for_clip(world, component);
        if let Some(handle) = Self::find_stencil_clip_renderable_handle(world, component) {
            visuals.register_stencil_clip(handle, stencil_ref);
        }

        if let Some(scope_root) = Self::stencil_clip_scope_root(world, component) {
            self.sync_stencil_refs_in_subtree(world, visuals, scope_root);
        }

        self.maybe_emit_panel_clip_debug(world, repl_command_queue);
    }

    pub fn unregister_stencil_clip(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.active_stencil_clips.remove(&component);

        if let Some(handle) = Self::find_stencil_clip_renderable_handle(world, component) {
            visuals.unregister_stencil_clip(handle);
        }

        if let Some(scope_root) = Self::stencil_clip_scope_root(world, component) {
            self.sync_stencil_refs_in_subtree(world, visuals, scope_root);
        }
    }

    pub fn unregister_stencil_clip_for_subtree_node(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.active_stencil_clips.remove(&component);
        if let Some(handle) = Self::find_stencil_clip_renderable_handle(world, component) {
            visuals.unregister_stencil_clip(handle);
        }
    }

    pub fn resync_after_renderable_flush(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        repl_command_queue: &mut Vec<String>,
    ) {
        let stencil_clips: Vec<ComponentId> = self.active_stencil_clips(world).collect();

        for stencil_clip in stencil_clips {
            let stencil_ref = Self::stencil_ref_for_clip(world, stencil_clip);
            if let Some(handle) = Self::find_stencil_clip_renderable_handle(world, stencil_clip) {
                let _ = visuals.register_stencil_clip(handle, stencil_ref);
            }
            if let Some(scope_root) = Self::stencil_clip_scope_root(world, stencil_clip) {
                self.sync_stencil_refs_in_subtree(world, visuals, scope_root);
            }
        }

        self.maybe_emit_panel_clip_debug(world, repl_command_queue);
    }

    fn sync_renderable_stencil_ref(
        &self,
        world: &World,
        visuals: &mut VisualWorld,
        renderable_component: ComponentId,
    ) {
        let Some(renderable) = world.get_component_by_id_as::<RenderableComponent>(renderable_component)
        else {
            return;
        };
        let Some(handle) = renderable.get_handle() else {
            return;
        };

        let stencil_ref = self
            .stencil_clip_for_renderable_component(world, renderable_component)
            .map(|clip_component| Self::stencil_ref_for_clip(world, clip_component))
            .unwrap_or_else(|| Self::stencil_ref_for_renderable(world, renderable_component));
        let _ = visuals.update_stencil_ref(handle, stencil_ref);
    }

    fn stencil_clip_for_renderable_component(
        &self,
        world: &World,
        renderable_component: ComponentId,
    ) -> Option<ComponentId> {
        self.active_stencil_clips.iter().copied().find(|&clip_component| {
            world.get_component_record(clip_component).is_some()
                && Self::find_stencil_clip_renderable_component(world, clip_component)
                    == Some(renderable_component)
        })
    }

    fn sync_stencil_refs_in_subtree(
        &self,
        world: &World,
        visuals: &mut VisualWorld,
        root: ComponentId,
    ) {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
                self.sync_renderable_stencil_ref(world, visuals, node);
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
    }

    fn active_stencil_clips<'a>(&'a mut self, world: &World) -> impl Iterator<Item = ComponentId> + 'a {
        self.active_stencil_clips
            .retain(|&component| world.get_component_record(component).is_some());
        self.active_stencil_clips.iter().copied()
    }

    fn stencil_ref_for_renderable(world: &World, renderable_component: ComponentId) -> u8 {
        use crate::engine::ecs::component::StencilClipComponent;

        let mut depth: u8 = 0;
        let mut cursor = Some(renderable_component);
        while let Some(node) = cursor {
            if world.get_component_by_id_as::<StencilClipComponent>(node).is_some()
                || Self::is_layout_clip_scope_root(world, node)
            {
                depth = depth.saturating_add(1);
            }
            cursor = world.parent_of(node);
        }
        depth
    }

    fn stencil_ref_for_clip(world: &World, component: ComponentId) -> u8 {
        use crate::engine::ecs::component::StencilClipComponent;

        let mut depth: u8 = 0;
        let mut cursor = if Self::is_layout_owned_stencil_clip(world, component) {
            world
                .parent_of(component)
                .and_then(|scope_root| world.parent_of(scope_root))
        } else {
            world.parent_of(component)
        };

        while let Some(node) = cursor {
            if world.get_component_by_id_as::<StencilClipComponent>(node).is_some()
                || Self::is_layout_clip_scope_root(world, node)
            {
                depth = depth.saturating_add(1);
            }
            cursor = world.parent_of(node);
        }

        depth
    }

    fn is_layout_owned_stencil_clip(world: &World, component: ComponentId) -> bool {
        world.component_label(component) == Some(OWNED_LAYOUT_STENCIL_CLIP_LABEL)
            && world
                .get_component_by_id_as::<crate::engine::ecs::component::StencilClipComponent>(component)
                .is_some()
    }

    fn immediate_owned_layout_stencil_clip(world: &World, scope_root: ComponentId) -> Option<ComponentId> {
        world.children_of(scope_root).iter().copied().find(|&child| {
            Self::is_layout_owned_stencil_clip(world, child)
        })
    }

    fn layout_bg_node(world: &World, scope_root: ComponentId) -> Option<ComponentId> {
        world.children_of(scope_root)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some("__bg"))
    }

    fn subtree_first_renderable(world: &World, root: ComponentId) -> Option<ComponentId> {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.get_component_by_id_as::<RenderableComponent>(node).is_some() {
                return Some(node);
            }
            for &child in world.children_of(node).iter().rev() {
                stack.push(child);
            }
        }
        None
    }

    fn is_layout_clip_scope_root(world: &World, node: ComponentId) -> bool {
        Self::immediate_owned_layout_stencil_clip(world, node).is_some()
    }

    fn stencil_clip_scope_root(world: &World, component: ComponentId) -> Option<ComponentId> {
        if Self::is_layout_owned_stencil_clip(world, component) {
            return world.parent_of(component);
        }
        let parent = world.parent_of(component)?;
        if world.component_label(parent) == Some("__bg") {
            return world.parent_of(parent);
        }
        Some(parent)
    }

    fn maybe_emit_panel_clip_debug(&mut self, world: &World, repl_command_queue: &mut Vec<String>) {
        static DID_EMIT_PANEL_CLIP_DEBUG: AtomicBool = AtomicBool::new(false);

        let want_paths = Self::env_flag("CAT_DEBUG_PANEL_CLIP_PATHS");
        let want_repl = Self::env_flag("CAT_DEBUG_PANEL_CLIP_REPL");

        if !want_paths && !want_repl {
            return;
        }

        if DID_EMIT_PANEL_CLIP_DEBUG.load(Ordering::Relaxed) {
            return;
        }

        let Some(world_info) = Self::panel_clip_debug_info::<
            crate::engine::ecs::component::WorldPanelComponent,
        >(world, "world_panel") else {
            return;
        };
        let Some(inspector_info) = Self::panel_clip_debug_info::<
            crate::engine::ecs::component::InspectorPanelComponent,
        >(world, "inspector_panel") else {
            return;
        };

        if want_paths {
            Self::print_panel_clip_debug_info(world, &world_info);
            Self::print_panel_clip_debug_info(world, &inspector_info);
        }

        if want_repl {
            repl_command_queue.push(format!("cd {}", world_info.renderable_guid));
            repl_command_queue.push("pwd".to_string());
            repl_command_queue.push(format!("cd {}", inspector_info.renderable_guid));
            repl_command_queue.push("pwd".to_string());
        }

        DID_EMIT_PANEL_CLIP_DEBUG.store(true, Ordering::Relaxed);
    }

    fn panel_clip_debug_info<T: crate::engine::ecs::component::Component + 'static>(
        world: &World,
        label: &str,
    ) -> Option<PanelClipDebugInfo> {
        let Some(panel_component) = world
            .all_components()
            .find(|&cid| world.get_component_by_id_as::<T>(cid).is_some())
        else {
            return None;
        };

        let Some((scope_root, bg_id, stencil_clip_id, renderable_id)) =
            Self::panel_clip_debug_nodes(world, panel_component)
        else {
            return None;
        };

        let renderable_guid = world.get_component_node(renderable_id)?.guid;

        Some(PanelClipDebugInfo {
            label: label.to_string(),
            scope_root,
            bg_id,
            stencil_clip_id,
            renderable_id,
            renderable_guid,
        })
    }

    fn print_panel_clip_debug_info(world: &World, info: &PanelClipDebugInfo) {
        println!(
            "[StencilClipDebug] {}: scope=\"{}\" bg=\"{}\" clip=\"{}\" renderable=\"{}\" guid={}",
            info.label,
            Self::repl_path_for_component(world, info.scope_root),
            Self::repl_path_for_component(world, info.bg_id),
            Self::repl_path_for_component(world, info.stencil_clip_id),
            Self::repl_path_for_component(world, info.renderable_id),
            info.renderable_guid,
        );
    }

    fn panel_clip_debug_nodes(
        world: &World,
        panel_component: ComponentId,
    ) -> Option<(ComponentId, ComponentId, ComponentId, ComponentId)> {
        let scope_root = world.parent_of(panel_component)?;
        let bg_id = Self::layout_bg_node(world, scope_root)?;
        let stencil_clip_id = Self::immediate_owned_layout_stencil_clip(world, scope_root)?;
        let renderable_id = Self::find_stencil_clip_renderable_component(world, stencil_clip_id)?;
        Some((scope_root, bg_id, stencil_clip_id, renderable_id))
    }

    fn find_stencil_clip_renderable_component(
        world: &World,
        component: ComponentId,
    ) -> Option<ComponentId> {
        let mut cursor = world.parent_of(component);
        while let Some(cid) = cursor {
            if world.get_component_by_id_as::<RenderableComponent>(cid).is_some() {
                return Some(cid);
            }
            cursor = world.parent_of(cid);
        }

        if Self::is_layout_owned_stencil_clip(world, component) {
            let scope_root = world.parent_of(component)?;
            let bg_id = Self::layout_bg_node(world, scope_root)?;
            return Self::subtree_first_renderable(world, bg_id);
        }

        None
    }

    fn find_stencil_clip_renderable_handle(
        world: &World,
        component: ComponentId,
    ) -> Option<crate::engine::graphics::primitives::InstanceHandle> {
        let renderable_component = Self::find_stencil_clip_renderable_component(world, component)?;
        world
            .get_component_by_id_as::<RenderableComponent>(renderable_component)
            .and_then(|r| r.get_handle())
    }

    fn env_flag(name: &str) -> bool {
        std::env::var(name)
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false)
    }

    fn format_component_id_short(id: ComponentId) -> String {
        let s = format!("{:?}", id);
        if let (Some(l), Some(r)) = (s.find('('), s.rfind(')')) {
            if r > l + 1 {
                return s[l + 1..r].to_string();
            }
        }
        s
    }

    fn repl_path_for_component(world: &World, component: ComponentId) -> String {
        let mut parts = Vec::new();
        let mut cursor = Some(component);

        while let Some(cid) = cursor {
            let name = world
                .get_component_node(cid)
                .map(|node| node.name.clone())
                .unwrap_or_else(|| "<deleted>".to_string());
            parts.push(format!("{}:{}", Self::format_component_id_short(cid), name));
            cursor = world.parent_of(cid);
        }

        parts.reverse();
        format!("/{}", parts.join("/"))
    }
}
