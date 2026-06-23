//! `ComponentRef` — an unresolved reference to another component, plus
//! the surface form the author wrote.
//!
//! Used by components that hold pointers to other components in MMS
//! source (`ActionComponent.target_sources`, `IKChainComponent.target_source`,
//! `IKChainComponent.end_effector_source`, …). The two variants cover the
//! two durable on-disk forms; whichever was authored is preserved
//! verbatim through dump so save → reload reproduces the original
//! source.
//!
//! - `Guid(uuid)` — author wrote `@uuid:<hex>` OR passed a live
//!   `Value::ComponentObject` (let-bound / query result), which the
//!   registry collapses to the target's guid at call-construction time.
//!   Resolution at runtime is an O(1) hashmap hit via
//!   `World::component_id_by_guid`.
//! - `Query(selector)` — any other selector string (`#name`,
//!   `[attr=value]`, `../#name`, `/#name`, ...). Preserved as-is;
//!   resolution may treat a leading root prefix as a scope override.
//!
//! Resolution is typically deferred — components carry both a
//! `ComponentRef` (for dump) and a resolved `ComponentId` (for runtime).
//! A system pass fills the resolved id when the referent is reachable;
//! both `AnimationSystem` (for Action) and `IKSystem` (for IKChain) do
//! this just before consuming the resolved id.

use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentRef {
    Guid(uuid::Uuid),
    Query(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryRootMode {
    SelfSubtree,
    ParentScope { levels_up: usize },
    WorldRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopedQuery<'a> {
    pub root_mode: QueryRootMode,
    pub selector: &'a str,
}

pub fn parse_scoped_query(input: &str) -> ScopedQuery<'_> {
    let mut rest = input;
    let mut levels_up = 0usize;
    while let Some(next) = rest.strip_prefix("../") {
        levels_up += 1;
        rest = next.trim_start();
    }
    if levels_up > 0 {
        return ScopedQuery {
            root_mode: QueryRootMode::ParentScope { levels_up },
            selector: rest,
        };
    }
    if let Some(next) = rest.strip_prefix('/') {
        return ScopedQuery {
            root_mode: QueryRootMode::WorldRoot,
            selector: next.trim_start(),
        };
    }
    ScopedQuery {
        root_mode: QueryRootMode::SelfSubtree,
        selector: input,
    }
}

pub fn resolve_component_ref(
    world: &World,
    src: &ComponentRef,
    owner: Option<ComponentId>,
    default_root_mode: QueryRootMode,
) -> Option<ComponentId> {
    match src {
        ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
        ComponentRef::Query(query) => resolve_scoped_query(world, owner, query, default_root_mode),
    }
}

pub fn resolve_scoped_query(
    world: &World,
    owner: Option<ComponentId>,
    query: &str,
    default_root_mode: QueryRootMode,
) -> Option<ComponentId> {
    let scoped = parse_scoped_query(query);
    let root_mode = match scoped.root_mode {
        QueryRootMode::SelfSubtree if query == scoped.selector => default_root_mode,
        other => other,
    };
    let selector = scoped.selector.trim();
    if selector.is_empty() {
        return None;
    }

    match root_mode {
        QueryRootMode::SelfSubtree => {
            let root = owner?;
            world.find_component(root, selector)
        }
        QueryRootMode::ParentScope { levels_up } => {
            let root = climb_scope(world, owner?, levels_up)?;
            world.find_component(root, selector)
        }
        QueryRootMode::WorldRoot => resolve_world_root_query(world, selector),
    }
}

fn climb_scope(world: &World, mut current: ComponentId, levels_up: usize) -> Option<ComponentId> {
    for _ in 0..levels_up {
        current = world.parent_of(current)?;
    }
    Some(current)
}

fn resolve_world_root_query(world: &World, selector: &str) -> Option<ComponentId> {
    if let Some(stripped) = selector.strip_prefix('>') {
        let child_selector = stripped.trim_start();
        if child_selector.is_empty() {
            return None;
        }
        return world
            .world_roots()
            .into_iter()
            .find(|&root| world.component_matches_selector(root, child_selector));
    }
    world
        .world_roots()
        .into_iter()
        .find_map(|root| world.find_component(root, selector))
}
