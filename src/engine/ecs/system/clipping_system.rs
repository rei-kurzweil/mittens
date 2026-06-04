use crate::engine::ecs::component::RenderableComponent;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::graphics::VisualWorld;
use std::collections::HashSet;

pub(crate) const OWNED_LAYOUT_STENCIL_CLIP_LABEL: &str = "__layout_stencil_clip";

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
    ) {
        self.active_stencil_clips.insert(component);

        let stencil_ref = Self::stencil_ref_for_clip(world, component);
        if let Some(handle) = Self::find_stencil_clip_renderable_handle(world, component) {
            visuals.register_stencil_clip(handle, stencil_ref);
        }

        if let Some(scope_root) = Self::stencil_clip_scope_root(world, component) {
            self.sync_stencil_refs_in_subtree(world, visuals, scope_root);
        }
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

    pub fn resync_after_renderable_flush(&mut self, world: &World, visuals: &mut VisualWorld) {
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
    }

    fn sync_renderable_stencil_ref(
        &self,
        world: &World,
        visuals: &mut VisualWorld,
        renderable_component: ComponentId,
    ) {
        let Some(renderable) =
            world.get_component_by_id_as::<RenderableComponent>(renderable_component)
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
        self.active_stencil_clips
            .iter()
            .copied()
            .find(|&clip_component| {
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
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_some()
            {
                self.sync_renderable_stencil_ref(world, visuals, node);
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
    }

    fn active_stencil_clips<'a>(
        &'a mut self,
        world: &World,
    ) -> impl Iterator<Item = ComponentId> + 'a {
        self.active_stencil_clips
            .retain(|&component| world.get_component_record(component).is_some());
        self.active_stencil_clips.iter().copied()
    }

    fn stencil_ref_for_renderable(world: &World, renderable_component: ComponentId) -> u8 {
        use crate::engine::ecs::component::StencilClipComponent;

        let mut depth: u8 = 0;
        let mut cursor = Some(renderable_component);
        while let Some(node) = cursor {
            if world
                .get_component_by_id_as::<StencilClipComponent>(node)
                .is_some()
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
            if world
                .get_component_by_id_as::<StencilClipComponent>(node)
                .is_some()
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
                .get_component_by_id_as::<crate::engine::ecs::component::StencilClipComponent>(
                    component,
                )
                .is_some()
    }

    fn immediate_owned_layout_stencil_clip(
        world: &World,
        scope_root: ComponentId,
    ) -> Option<ComponentId> {
        world
            .children_of(scope_root)
            .iter()
            .copied()
            .find(|&child| Self::is_layout_owned_stencil_clip(world, child))
    }

    fn layout_bg_node(world: &World, scope_root: ComponentId) -> Option<ComponentId> {
        world
            .children_of(scope_root)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some("__bg"))
    }

    fn subtree_first_renderable(world: &World, root: ComponentId) -> Option<ComponentId> {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world
                .get_component_by_id_as::<RenderableComponent>(node)
                .is_some()
            {
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

    fn find_stencil_clip_renderable_component(
        world: &World,
        component: ComponentId,
    ) -> Option<ComponentId> {
        let mut cursor = world.parent_of(component);
        while let Some(cid) = cursor {
            if world
                .get_component_by_id_as::<RenderableComponent>(cid)
                .is_some()
            {
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
}
