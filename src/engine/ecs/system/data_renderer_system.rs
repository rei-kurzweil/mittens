use std::collections::HashMap;

use crate::engine::ecs::component::{
    Display, LayoutComponent, Overflow, SizeDimension, StyleComponent, TransformComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::meow_meow::component_registry::spawn_tree;
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::MeowMeowRunner;

// ── Payload types ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiItemKind {
    Component,
    Info,
    EditorRoot,
    Spacer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiItem {
    pub key: String,
    pub kind: UiItemKind,
    pub label: String,
    pub selected: bool,
    pub target_ref: Option<ComponentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiDetailItem {
    pub name: String,
    pub id: String,
    pub guid: String,
}

// ── Renderer spec ──

/// How to render one unit of data into a live component subtree.
pub enum RendererSpec<T> {
    /// Materialize an MMS component expression and spawn it.
    Mms {
        asset_path: &'static str,
        export_name: &'static str,
        to_args: fn(&T) -> Vec<Value>,
    },
    /// Build a component subtree directly from Rust.
    ///
    /// Uses a boxed closure so the renderer can capture shared state
    /// (e.g. panel models from an `Arc<Mutex<Vec<...>>>`).
    Rust {
        render_fn: Box<
            dyn Fn(&mut World, &mut dyn SignalEmitter, &T) -> Result<ComponentId, String>
                + Send
                + Sync,
        >,
    },
}

pub type ItemRendererSpec = RendererSpec<UiItem>;
pub type DetailRendererSpec = RendererSpec<UiDetailItem>;

// ── System ──

/// Owns the data-to-live-subtree projection lifecycle for editor UI slots.
///
/// Tracks rendered subtrees per slot so that subsequent renders can remove
/// previous content before attaching new content (full-rerender semantics).
#[derive(Debug, Default)]
pub struct DataRendererSystem {
    rendered_subtrees: HashMap<ComponentId, ComponentId>,
}

impl DataRendererSystem {
    pub fn new() -> Self {
        Self {
            rendered_subtrees: HashMap::new(),
        }
    }

    /// Render a list of items into a target slot.
    ///
    /// Each item is rendered independently via `spec`. Results are collected
    /// under a container that becomes the slot's single child.
    /// Any previously rendered subtree for this slot is removed first.
    ///
    /// Returns the container `ComponentId` so callers can attach additional
    /// panel-specific state (e.g. `SelectionComponent`).
    pub fn render_list(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        slot: ComponentId,
        spec: &ItemRendererSpec,
        items: &[UiItem],
    ) -> Result<ComponentId, String> {
        self.remove_previous(world, emit, slot);
        let container = spawn_list_container(world, slot);

        for item in items {
            let child_id = render_item(world, emit, spec, item)?;
            let _ = world.add_child(container, child_id);
        }

        emit.push_intent_now(
            container,
            IntentValue::Attach {
                parents: vec![slot],
                child: container,
            },
        );
        self.rendered_subtrees.insert(slot, container);
        mark_nearest_layout_dirty(world, slot);
        Ok(container)
    }

    /// Render a detail view into a target slot.
    ///
    /// Any previously rendered subtree for this slot is removed first.
    ///
    /// Returns the root `ComponentId` of the rendered subtree.
    pub fn render_detail(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        slot: ComponentId,
        spec: &DetailRendererSpec,
        detail: &UiDetailItem,
    ) -> Result<ComponentId, String> {
        self.remove_previous(world, emit, slot);
        let root = render_detail_item(world, emit, spec, detail)?;

        emit.push_intent_now(
            root,
            IntentValue::Attach {
                parents: vec![slot],
                child: root,
            },
        );
        self.rendered_subtrees.insert(slot, root);
        mark_nearest_layout_dirty(world, slot);
        Ok(root)
    }

    /// Remove any rendered content for this slot. No-op if slot is not tracked.
    pub fn clear_slot(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        slot: ComponentId,
    ) {
        self.remove_previous(world, emit, slot);
    }

    fn remove_previous(&mut self, world: &World, emit: &mut dyn SignalEmitter, slot: ComponentId) {
        if let Some(prev_root) = self.rendered_subtrees.remove(&slot) {
            if world.get_component_record(prev_root).is_some() {
                emit.push_intent_now(
                    prev_root,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![prev_root],
                    },
                );
            }
        }
    }
}

// ── Internal render dispatch ──

fn render_item(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    spec: &ItemRendererSpec,
    item: &UiItem,
) -> Result<ComponentId, String> {
    match spec {
        RendererSpec::Mms {
            asset_path,
            export_name,
            to_args,
        } => {
            let args = (to_args)(item);
            let ce = MeowMeowRunner::materialize_mms_module_component_from_file(
                asset_path,
                export_name,
                args,
                Some(world),
                Some(emit),
            )
            .map_err(|e| format!("MMS materialization failed: {e}"))?;
            spawn_tree(&ce, None, world, emit).map_err(|e| format!("spawn_tree failed: {e}"))
        }
        RendererSpec::Rust { render_fn } => (render_fn)(world, emit, item),
    }
}

fn render_detail_item(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    spec: &DetailRendererSpec,
    detail: &UiDetailItem,
) -> Result<ComponentId, String> {
    match spec {
        RendererSpec::Mms {
            asset_path,
            export_name,
            to_args,
        } => {
            let args = (to_args)(detail);
            let ce = MeowMeowRunner::materialize_mms_module_component_from_file(
                asset_path,
                export_name,
                args,
                Some(world),
                Some(emit),
            )
            .map_err(|e| format!("MMS materialization failed: {e}"))?;
            spawn_tree(&ce, None, world, emit).map_err(|e| format!("spawn_tree failed: {e}"))
        }
        RendererSpec::Rust { render_fn } => (render_fn)(world, emit, detail),
    }
}

// ── Helpers ──

fn spawn_list_container(world: &mut World, slot: ComponentId) -> ComponentId {
    let name = format!("data_renderer_list_{slot:?}");
    let root = world.add_component_boxed_named(&name, Box::new(TransformComponent::new()));
    let style = world.add_component_boxed_named(
        format!("{name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(root, style);
    root
}

fn mark_nearest_layout_dirty(world: &mut World, start: ComponentId) {
    let mut current = Some(start);
    while let Some(id) = current {
        if let Some(layout) = world.get_component_by_id_as_mut::<LayoutComponent>(id) {
            layout.mark_dirty();
            return;
        }
        current = world.parent_of(id);
    }
}
