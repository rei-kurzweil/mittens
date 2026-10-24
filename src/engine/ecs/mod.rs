pub mod command_queue;
pub mod component;
pub mod signals;
pub mod system;
pub mod world_query_adapter;

#[cfg(test)]
mod world_graph_tests;

use crate::engine::graphics::{RenderAssets, VisualWorld};
use slotmap::{SlotMap, new_key_type};
use std::cell::RefCell;
use std::collections::HashMap;

new_key_type! {
    /// Global component identity (dense arena key).
    pub struct ComponentId;
}

/// Reference to one World-owned structural attachment, not a mount relationship.
/// Reattaching the same endpoints creates a new revision; stale references never
/// acquire ownership of that replacement. ComponentId also guards slot reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentRef {
    parent: ComponentId,
    child: ComponentId,
    revision: u64,
}

impl AttachmentRef {
    pub fn parent(self) -> ComponentId {
        self.parent
    }

    pub fn child(self) -> ComponentId {
        self.child
    }
}

// Re-export these so other modules can use `crate::engine::ecs::Transform`
// and `crate::engine::ecs::Renderable` consistently.
#[allow(unused_imports)]
pub use crate::engine::graphics::primitives::{Renderable, Transform, TransformMatrix};

pub use command_queue::CommandQueue;
pub use signals::{
    EventSignal, IntentSignal, IntentValue, KeyboardEvent, PointerActivationSource, PoseApplyMode,
    RxWorld, Signal, SignalEmitter, SignalHandler, SignalKind, SignalWhen,
};
pub use system::{System, SystemWorld};
pub use world_query_adapter::WorldQueryAdapter;

/// Bundle of mutable engine state passed to component mutation APIs.
///
/// This exists to avoid threading `&mut World`, `&mut SystemWorld`, and `&mut VisualWorld`
/// through every component call.
pub struct WorldContext<'a> {
    pub world: &'a mut World,
    pub systems: &'a mut SystemWorld,
    pub visuals: &'a mut VisualWorld,
    pub render_assets: &'a mut RenderAssets,
}

impl<'a> WorldContext<'a> {
    pub fn new(
        world: &'a mut World,
        systems: &'a mut SystemWorld,
        visuals: &'a mut VisualWorld,
        render_assets: &'a mut RenderAssets,
    ) -> Self {
        Self {
            world,
            systems,
            visuals,
            render_assets,
        }
    }
}

/// World: owns all global components.
#[derive(Default)]
pub struct World {
    /// Invalidates retained selector results on topology/label/GUID changes.
    query_revision: u64,
    /// Typed contact registry: discovery never walks unrelated scene components.
    collidable_ids: std::collections::HashSet<ComponentId>,
    /// Consumed by the simulation's StaticContactSystem after pose propagation.
    contact_changes: RefCell<std::collections::HashSet<ComponentId>>,
    components: SlotMap<ComponentId, crate::engine::ecs::component::ComponentNode>,
    /// Additional incoming edges for shared leaf Shading declarations. Ordinary
    /// trees retain their inline parent and allocate no adjacency side storage.
    additional_parents: HashMap<ComponentId, Vec<AttachmentRef>>,
    attachment_sequence: u64,
    shading_topology_changes: std::collections::HashSet<ComponentId>,
    material_changes: std::collections::HashSet<ComponentId>,
    guid_index: HashMap<uuid::Uuid, ComponentId>,
    /// MMQ parser instance with per-instance AST cache. Behind a RefCell so
    /// `find_component(&self, ...)` can mutate the cache without forcing
    /// every caller to thread `&mut World`. Single-threaded — World lives
    /// on the main thread.
    mmq_parser: RefCell<mittens_query::mmq::MmqQuerySyntax>,
}

impl World {
    /// Fast GUID -> ComponentId lookup.
    pub fn component_id_by_guid(&self, guid: uuid::Uuid) -> Option<ComponentId> {
        self.guid_index.get(&guid).copied()
    }

    /// Replace the GUID of an already-inserted component, keeping its
    /// ComponentId stable. Used by spawn paths that restore an authored
    /// `guid = "..."` property after `create_component` has already minted
    /// a fresh GUID.
    ///
    /// Returns Err if the target id is missing, or if the new guid is
    /// already taken by another component (would silently overwrite the
    /// reverse index otherwise).
    pub fn set_component_guid(
        &mut self,
        id: ComponentId,
        new_guid: uuid::Uuid,
    ) -> Result<(), String> {
        let Some(node) = self.get_component_record(id) else {
            return Err(format!("set_component_guid: component {id:?} missing"));
        };
        let old_guid = node.guid;
        if old_guid == new_guid {
            return Ok(());
        }
        if let Some(&existing) = self.guid_index.get(&new_guid) {
            if existing != id {
                return Err(format!(
                    "set_component_guid: guid {new_guid} already in use by {existing:?}"
                ));
            }
        }
        self.guid_index.remove(&old_guid);
        self.guid_index.insert(new_guid, id);
        if let Some(node) = self.get_component_record_mut(id) {
            node.guid = new_guid;
        }
        Ok(())
    }

    /// Add a new component to the world (no parent) and return its id.
    ///
    /// Note: this currently does *not* call `Component::init`. That should happen via a
    /// higher-level API that has access to `SystemWorld` and `VisualWorld`.
    pub fn add_component<T: crate::engine::ecs::component::Component>(
        &mut self,
        c: T,
    ) -> ComponentId {
        // We set the id after insertion so components that cache their id can do so.
        let id = self.add_component_boxed(Box::new(c));
        if let Some(node) = self.get_component_record_mut(id) {
            node.component.set_id(id);
        }
        id
    }

    /// Register a new component in the world and return its id.
    ///
    /// This is intentionally a storage/identity operation only: it does *not* call
    /// `Component::init`.
    pub fn register<T: crate::engine::ecs::component::Component>(&mut self, c: T) -> ComponentId {
        self.add_component(c)
    }

    /// Whether this component has had `Component::init` invoked.
    pub fn is_initialized(&self, c: ComponentId) -> bool {
        self.get_component_record(c)
            .map(|n| n.initialized)
            .unwrap_or(false)
    }

    /// Add a new component to the world (no parent). Returns its global id.
    pub fn add_component_boxed(
        &mut self,
        c: Box<dyn crate::engine::ecs::component::Component>,
    ) -> ComponentId {
        let node = crate::engine::ecs::component::ComponentNode::new(c);
        let guid = node.guid;
        let id = self.components.insert(node);
        self.track_contact_component(id);
        let _old = self.guid_index.insert(guid, id);
        if let Some(node) = self.get_component_record_mut(id) {
            node.component.set_id(id);
        }
        id
    }

    /// Add a new boxed component with an explicit stored name.
    pub fn add_component_boxed_named(
        &mut self,
        name: impl Into<String>,
        c: Box<dyn crate::engine::ecs::component::Component>,
    ) -> ComponentId {
        let node = crate::engine::ecs::component::ComponentNode::new_named(name, c);
        let guid = node.guid;
        let id = self.components.insert(node);
        self.track_contact_component(id);
        let _old = self.guid_index.insert(guid, id);
        if let Some(node) = self.get_component_record_mut(id) {
            node.component.set_id(id);
        }
        id
    }

    /// Add a new boxed component with a restored GUID and explicit stored name.
    ///
    /// This is intended for deserialization.
    pub fn add_component_boxed_with_guid_named(
        &mut self,
        guid: uuid::Uuid,
        name: impl Into<String>,
        c: Box<dyn crate::engine::ecs::component::Component>,
    ) -> ComponentId {
        if self.guid_index.contains_key(&guid) {
            panic!("duplicate component guid inserted into World: {}", guid);
        }

        let node = crate::engine::ecs::component::ComponentNode::new_with_guid_named(guid, name, c);
        let guid = node.guid;
        let id = self.components.insert(node);
        self.track_contact_component(id);
        self.guid_index.insert(guid, id);
        if let Some(node) = self.get_component_record_mut(id) {
            node.component.set_id(id);
        }
        id
    }

    /// Temporary alias during migration.
    pub fn spawn_component_boxed(
        &mut self,
        c: Box<dyn crate::engine::ecs::component::Component>,
    ) -> ComponentId {
        self.add_component_boxed(c)
    }

    pub fn get_component_record(
        &self,
        id: ComponentId,
    ) -> Option<&crate::engine::ecs::component::ComponentNode> {
        self.components.get(id)
    }

    /// Alias for `get_component_record`.
    pub fn get_component_node(
        &self,
        id: ComponentId,
    ) -> Option<&crate::engine::ecs::component::ComponentNode> {
        self.get_component_record(id)
    }

    pub fn query_revision(&self) -> u64 {
        self.query_revision
    }

    fn track_contact_component(&mut self, id: ComponentId) {
        if self
            .get_component_by_id_as::<component::CollidableComponent>(id)
            .is_some()
        {
            self.collidable_ids.insert(id);
        }
        self.contact_changes.get_mut().insert(id);
    }

    pub(crate) fn collidable_ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.collidable_ids.iter().copied()
    }

    pub(crate) fn take_contact_changes(&self) -> std::collections::HashSet<ComponentId> {
        std::mem::take(&mut *self.contact_changes.borrow_mut())
    }

    pub fn get_component_record_mut(
        &mut self,
        id: ComponentId,
    ) -> Option<&mut crate::engine::ecs::component::ComponentNode> {
        // Raw record access can change selector metadata or topology. Ordinary
        // typed component writes bypass this conservative invalidation.
        self.query_revision = self.query_revision.wrapping_add(1);
        self.contact_changes.get_mut().insert(id);
        self.components.get_mut(id)
    }

    /// Returns the engine type identifier for this component (e.g. `"transform"`).
    pub fn component_name(&self, id: ComponentId) -> Option<&str> {
        self.get_component_record(id)
            .map(|node| node.component_type.as_str())
    }

    /// Returns the user-assigned label for this component (empty string if unset).
    pub fn component_label(&self, id: ComponentId) -> Option<&str> {
        self.get_component_record(id).map(|node| node.name.as_str())
    }

    // --- Topology helpers (component-graph) ---
    /// Unique incoming parent. Returns None for roots and ambiguous shared
    /// declarations; use parents_of for attachment-aware traversal.
    pub fn parent_of(&self, c: ComponentId) -> Option<ComponentId> {
        // There is no implicit "first parent" for a shared declaration.
        let node = self.get_component_record(c)?;
        if node.has_additional_parents {
            return None;
        }
        node.parent
    }

    /// All attachment parents, without allocating. Shared declarations do not
    /// inherit a transform or scope from an arbitrarily selected parent.
    pub fn parents_of(&self, c: ComponentId) -> impl Iterator<Item = ComponentId> + '_ {
        let node = self.get_component_record(c);
        let additional = node
            .filter(|n| n.has_additional_parents)
            .and_then(|_| self.additional_parents.get(&c));
        node.and_then(|n| n.parent)
            .into_iter()
            .chain(additional.into_iter().flatten().map(|edge| edge.parent))
    }

    pub fn attachment_between(
        &self,
        parent: ComponentId,
        child: ComponentId,
    ) -> Option<AttachmentRef> {
        let node = self.get_component_record(child)?;
        if node.parent == Some(parent) {
            return Some(AttachmentRef {
                parent,
                child,
                revision: node.attachment_revision,
            });
        }
        if !node.has_additional_parents {
            return None;
        }
        self.additional_parents
            .get(&child)?
            .iter()
            .find(|edge| edge.parent == parent)
            .copied()
    }

    pub(crate) fn take_shading_topology_changes(
        &mut self,
    ) -> std::collections::HashSet<ComponentId> {
        std::mem::take(&mut self.shading_topology_changes)
    }

    pub(crate) fn is_shading_declaration(&self, c: ComponentId) -> bool {
        self.get_component_by_id_as::<component::ShadingComponent>(c)
            .is_some()
            || self
                .get_component_by_id_as::<component::material::MaterialComponent>(c)
                .is_some()
    }

    pub(crate) fn mark_material_changed(&mut self, component: ComponentId) {
        self.material_changes.insert(component);
    }

    pub(crate) fn mark_material_scope_changed(&mut self, component: ComponentId) {
        let parents: Vec<_> = self.parents_of(component).collect();
        self.shading_topology_changes.extend(parents);
        self.shading_topology_changes.insert(component);
    }

    pub(crate) fn take_material_changes(&mut self) -> std::collections::HashSet<ComponentId> {
        std::mem::take(&mut self.material_changes)
    }

    /// Allocation-free reference to the current incoming attachment.
    pub fn attachment_of(&self, child: ComponentId) -> Option<AttachmentRef> {
        let node = self.get_component_record(child)?;
        if node.has_additional_parents {
            return None;
        }
        Some(AttachmentRef {
            parent: node.parent?,
            child,
            revision: node.attachment_revision,
        })
    }

    /// All World mutation paths invalidate references at the storage boundary,
    /// including direct mutations that do not emit runtime ParentChanged events.
    pub fn attachment_is_live(&self, attachment: AttachmentRef) -> bool {
        self.attachment_between(attachment.parent, attachment.child) == Some(attachment)
    }

    /// Iterator over all component IDs in the world.
    pub fn all_components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.keys()
    }

    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn children_of(&self, c: ComponentId) -> &[ComponentId] {
        static EMPTY: [ComponentId; 0] = [];
        self.get_component_record(c)
            .map(|n| n.children.as_slice())
            .unwrap_or(&EMPTY)
    }

    // --- Typed component access ---
    pub fn get_component_by_id_as<T: 'static>(&self, c: ComponentId) -> Option<&T> {
        let node = self.get_component_record(c)?;
        node.component.as_any().downcast_ref::<T>()
    }

    pub fn get_component_by_id_as_mut<T: 'static>(&mut self, c: ComponentId) -> Option<&mut T> {
        let node = self.components.get_mut(c)?;
        let typed = node.component.as_any_mut().downcast_mut::<T>()?;
        let kind = std::any::TypeId::of::<T>();
        if kind == std::any::TypeId::of::<component::TransformComponent>()
            || kind == std::any::TypeId::of::<component::ZoneComponent>()
            || kind == std::any::TypeId::of::<component::CollidableComponent>()
        {
            self.contact_changes.get_mut().insert(c);
        }
        Some(typed)
    }

    /// Find the first component under `root` matching `selector`.
    ///
    /// `selector` is parsed as MMQ — `#name`, `Type`, `Type#name`, `[name='...']`, etc.
    /// See `crates/mittens-query/src/mmq/parser.rs`.
    pub fn find_component(&self, root: ComponentId, selector: &str) -> Option<ComponentId> {
        let matches = self.run_query(root, selector);
        matches.into_iter().next()
    }

    /// Find all components under `root` matching `selector` (DFS pre-order).
    pub fn find_all_components(&self, root: ComponentId, selector: &str) -> Vec<ComponentId> {
        self.run_query(root, selector)
    }

    /// Roots searched by a live scripting query scoped to `component`.
    ///
    /// Imported glTF nodes are physically attached beneath the asset's transform anchor rather
    /// than the `GLTFComponent` itself. Include the top-level imported nodes recorded in runtime
    /// metadata so `gltf.query(...)` still has the expected asset-instance scope.
    pub fn scripting_query_roots(&self, component: ComponentId) -> Vec<ComponentId> {
        let Some(gltf) =
            self.get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(component)
        else {
            return vec![component];
        };

        let spawned: std::collections::HashSet<_> =
            gltf.spawned_node_transforms.iter().copied().collect();
        let mut roots = vec![component];
        roots.extend(gltf.spawned_node_transforms.iter().copied().filter(|node| {
            self.parent_of(*node)
                .is_none_or(|parent| !spawned.contains(&parent))
        }));
        roots
    }

    pub fn component_matches_selector(&self, component: ComponentId, selector: &str) -> bool {
        self.run_query(component, selector).first().copied() == Some(component)
    }

    pub fn world_roots(&self) -> Vec<ComponentId> {
        self.all_components()
            .filter(|&cid| self.parents_of(cid).next().is_none())
            .collect()
    }

    fn run_query(&self, root: ComponentId, selector: &str) -> Vec<ComponentId> {
        use crate::engine::ecs::world_query_adapter::WorldQueryAdapter;
        use mittens_query::QueryEvaluator;
        use mittens_query::QuerySyntax;

        if self.get_component_record(root).is_none() {
            return Vec::new();
        }
        let Ok(ast) = self.mmq_parser.borrow_mut().parse(selector) else {
            return Vec::new();
        };
        let adapter = WorldQueryAdapter::new(self);
        let matches = QueryEvaluator::evaluate(&adapter, root, &ast);
        if self.additional_parents.is_empty() {
            return matches;
        }
        let mut seen = std::collections::HashSet::new();
        matches.into_iter().filter(|id| seen.insert(*id)).collect()
    }

    pub fn get_parent_as<T: 'static>(&self, c: ComponentId) -> Option<(ComponentId, &T)> {
        let parent = self.parent_of(c)?;
        let typed = self.get_component_by_id_as::<T>(parent)?;
        Some((parent, typed))
    }

    pub fn get_parent_as_mut<T: 'static>(
        &mut self,
        c: ComponentId,
    ) -> Option<(ComponentId, &mut T)> {
        let parent = self.parent_of(c)?;
        // Avoid borrowing self twice by doing the downcast via the node.
        let typed = self.get_component_by_id_as_mut::<T>(parent)?;
        Some((parent, typed))
    }

    // --- Graph mutation ---
    fn is_ancestor_of(&self, maybe_ancestor: ComponentId, mut node: ComponentId) -> bool {
        while let Some(p) = self.parent_of(node) {
            if p == maybe_ancestor {
                return true;
            }
            node = p;
        }
        false
    }

    /// Attach `child` under `parent`.
    ///
    /// This is a lower-level graph mutation API.
    ///
    /// It updates only the parent/child links in the world graph. It does not
    /// emit `ParentChanged`, does not refresh topology-dependent systems, and
    /// does not route through runtime helpers like routers/scroll ownership.
    ///
    /// Prefer `IntentValue::Attach` or `Universe::attach(...)` when attaching a
    /// subtree into an already-live parent and you expect normal runtime side
    /// effects.
    ///
    /// `add_child(...)` is appropriate for offline subtree assembly before init,
    /// tests, and internal structural building where the caller intentionally
    /// manages any needed follow-up work.
    ///
    /// Safety rules:
    /// - Both ids must exist.
    /// - Structural children are reparented; leaf Shading declarations share.
    /// - Cycles are rejected.
    pub fn add_child(
        &mut self,
        parent: ComponentId,
        child: ComponentId,
    ) -> Result<(), &'static str> {
        let parent_node = self
            .get_component_record(parent)
            .ok_or("parent does not exist")?;
        if self.get_component_record(child).is_none() {
            return Err("child does not exist");
        }
        if parent_node.has_additional_parents {
            return Err("shared Shading declarations must remain leaves");
        }
        if parent == child {
            return Err("cannot parent component to itself");
        }
        if self.is_ancestor_of(child, parent) {
            return Err("cycle detected");
        }
        let shared_declaration = self.is_shading_declaration(child);
        if shared_declaration && self.parents_of(child).next().is_some() {
            if !self.children_of(child).is_empty() {
                return Err("only leaf Shading declarations support multiple parents");
            }
            if self.attachment_between(parent, child).is_some() {
                return Err("attachment already exists");
            }
        } else {
            self.detach_from_parent(child);
        }

        self.attachment_sequence = self
            .attachment_sequence
            .checked_add(1)
            .expect("attachment revision exhausted");
        let revision = self.attachment_sequence;
        if shared_declaration && self.get_component_record(child).unwrap().parent.is_some() {
            self.additional_parents
                .entry(child)
                .or_default()
                .push(AttachmentRef {
                    parent,
                    child,
                    revision,
                });
            self.get_component_record_mut(child)
                .unwrap()
                .has_additional_parents = true;
        } else {
            // Set child's parent.
            {
                let child_node = self
                    .get_component_record_mut(child)
                    .ok_or("child missing")?;
                child_node.parent = Some(parent);
                child_node.attachment_revision = revision;
            }
        }
        // Push into parent's children list.
        {
            let parent_node = self
                .get_component_record_mut(parent)
                .ok_or("parent missing")?;
            if !parent_node.children.contains(&child) {
                parent_node.children.push(child);
            }
        }

        if shared_declaration {
            self.shading_topology_changes.insert(parent);
        }

        Ok(())
    }

    /// Change a component's parent.
    ///
    /// Equivalent to `detach_from_parent(child)` when `new_parent` is None.
    pub fn set_parent(
        &mut self,
        child: ComponentId,
        new_parent: Option<ComponentId>,
    ) -> Result<(), &'static str> {
        if self
            .get_component_record(child)
            .is_some_and(|node| node.has_additional_parents)
        {
            return Err(
                "set_parent is ambiguous for a shared declaration; use detach_child or detach_all_parents",
            );
        }
        match new_parent {
            None => {
                self.detach_from_parent(child);
                Ok(())
            }
            Some(parent) => {
                // Explicit structural movement never adds another attachment.
                if self.is_shading_declaration(child) {
                    let parent_node = self
                        .get_component_record(parent)
                        .ok_or("parent does not exist")?;
                    if parent == child || self.is_ancestor_of(child, parent) {
                        return Err("cycle detected");
                    }
                    if parent_node.has_additional_parents {
                        return Err("shared Shading declarations must remain leaves");
                    }
                    self.detach_from_parent(child);
                }
                self.add_child(parent, child)
            }
        }
    }

    /// Detach `child` from its current parent.
    ///
    /// This does *not* delete anything.
    /// Ambiguous shared declarations are unchanged; use detach_child or
    /// detach_all_parents instead.
    pub fn detach_from_parent(&mut self, child: ComponentId) {
        let Some(old_parent) = self.parent_of(child) else {
            return;
        };
        self.detach_child(old_parent, child);
    }

    /// Remove exactly one edge, preserving the component and every other edge.
    pub fn detach_child(&mut self, parent: ComponentId, child: ComponentId) -> bool {
        if self.attachment_between(parent, child).is_none() {
            return false;
        }
        let node = self.get_component_record(child).unwrap();
        let inline = node.parent == Some(parent);
        let has_additional_parents = node.has_additional_parents;
        if inline {
            let replacement = if has_additional_parents {
                self.additional_parents
                    .get_mut(&child)
                    .and_then(|edges| edges.pop())
            } else {
                None
            };
            if let Some(node) = self.get_component_record_mut(child) {
                node.parent = replacement.map(|edge| edge.parent);
                if let Some(edge) = replacement {
                    node.attachment_revision = edge.revision;
                }
            }
        } else if let Some(edges) = self.additional_parents.get_mut(&child) {
            edges.retain(|edge| edge.parent != parent);
        }
        if has_additional_parents
            && self
                .additional_parents
                .get(&child)
                .is_some_and(Vec::is_empty)
        {
            self.additional_parents.remove(&child);
            self.get_component_record_mut(child)
                .unwrap()
                .has_additional_parents = false;
        }
        if let Some(node) = self.get_component_record_mut(parent) {
            node.children.retain(|&c| c != child);
        }
        if self.is_shading_declaration(child) {
            self.shading_topology_changes.insert(parent);
        }
        true
    }

    pub fn detach_all_parents(&mut self, child: ComponentId) {
        while let Some(parent) = self
            .get_component_record(child)
            .and_then(|node| node.parent)
        {
            self.detach_child(parent, child);
        }
    }

    /// Remove a component from the world.
    ///
    /// This is a *leaf-only* removal: it fails if the component still has children.
    /// Use `remove_component_subtree` when you want to delete a whole branch.
    pub fn remove_component_leaf(&mut self, c: ComponentId) -> Result<(), &'static str> {
        let guid = {
            let Some(node) = self.get_component_record(c) else {
                return Err("component does not exist");
            };
            if !node.children.is_empty() {
                return Err(
                    "component has children; use remove_component_subtree or detach children first",
                );
            }
            node.guid
        };

        self.guid_index.remove(&guid);

        self.detach_all_parents(c);
        self.components.remove(c);
        self.collidable_ids.remove(&c);
        self.contact_changes.get_mut().insert(c);
        self.query_revision = self.query_revision.wrapping_add(1);
        Ok(())
    }

    /// Remove a component and all its descendants.
    pub fn remove_component_subtree(&mut self, root: ComponentId) -> Result<(), &'static str> {
        if self.get_component_record(root).is_none() {
            return Err("component does not exist");
        }

        let nodes = self.owned_subtree(root);
        // Unlink outgoing shared edges even when the source survives elsewhere.
        for c in nodes.into_iter().rev() {
            let children = self.children_of(c).to_vec();
            for child in children {
                self.detach_child(c, child);
            }
            self.remove_component_leaf(c)?;
        }

        Ok(())
    }

    /// Destruction owns structural descendants, but shared declaration leaves
    /// survive when an incoming attachment originates outside this subtree.
    pub(crate) fn owned_subtree(&self, root: ComponentId) -> Vec<ComponentId> {
        let mut stack = vec![root];
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while let Some(node) = stack.pop() {
            if !seen.insert(node) {
                continue;
            }
            nodes.push(node);
            stack.extend(self.children_of(node).iter().copied());
        }
        nodes.retain(|&node| node == root || self.parents_of(node).all(|p| seen.contains(&p)));
        nodes
    }

    /// Initialize a component tree starting from the given root component.
    ///
    /// This recursively initializes the root component and all its descendants by calling
    /// `Component::init` on each component in the tree.
    pub fn init_component_tree(
        &mut self,
        root: ComponentId,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
    ) {
        use std::collections::HashSet;

        // Iterative traversal prevents stack overflows on large init expansions.
        let mut stack: Vec<ComponentId> = vec![root];
        let mut visited: HashSet<ComponentId> = HashSet::new();

        // Log only a small number of cycle detections to avoid spam.
        let mut cycle_logs_left: usize = 8;

        while let Some(node_id) = stack.pop() {
            if !visited.insert(node_id) {
                if cycle_logs_left > 0 {
                    cycle_logs_left -= 1;
                    println!(
                        "[World::init_component_tree] cycle/revisit detected at component={:?}",
                        node_id
                    );
                }
                continue;
            }

            // Initialize the component (idempotent).
            if let Some(node) = self.get_component_record_mut(node_id) {
                if !node.initialized {
                    node.component.init(emit, node_id);
                    node.initialized = true;
                }
            }

            // Push children (reverse so first child is processed first in LIFO order).
            let children: Vec<ComponentId> = self.children_of(node_id).to_vec();
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
    }
}
