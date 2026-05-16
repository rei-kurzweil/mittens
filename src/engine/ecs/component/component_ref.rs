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
//!   `[attr=value]`, ...). Preserved as-is; resolution walks the world
//!   roots via `World::find_component` (slow path).
//!
//! Resolution is typically deferred — components carry both a
//! `ComponentRef` (for dump) and a resolved `ComponentId` (for runtime).
//! A system pass fills the resolved id when the referent is reachable;
//! both `AnimationSystem` (for Action) and `IKSystem` (for IKChain) do
//! this just before consuming the resolved id.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentRef {
    Guid(uuid::Uuid),
    Query(String),
}
