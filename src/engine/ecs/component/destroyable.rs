use super::Component;
use crate::engine::ecs::system::triangle_collision::TriangleMesh;
use std::sync::Arc;

/// Opts exactly one sibling cube renderable and its sibling static box zone
/// into runtime destruction. Runtime damage never enters authored serialization.
#[derive(Debug, Clone, Copy)]
pub struct DestroyableComponent {
    pub threshold: f32,
    pub cell_size: f32,
    pub chunk_size: f32,
    pub(crate) exhausted: bool,
}
impl Default for DestroyableComponent {
    fn default() -> Self {
        Self {
            threshold: 3.,
            cell_size: 0.12,
            chunk_size: 1.0,
            exhausted: false,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct DamageComponent {
    pub position: [f32; 3],
    pub radius: f32,
    pub amount: f32,
}
/// Runtime precise geometry shared by a renderable and its zone.
#[derive(Debug, Clone)]
pub struct CutGeometryComponent {
    pub mesh: Arc<TriangleMesh>,
}
macro_rules! component {
    ($ty:ty, $name:literal, $body:expr) => {
        impl Component for $ty {
            fn name(&self) -> &'static str {
                $name
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn to_mms_ast(
                &self,
                _world: &crate::engine::ecs::World,
            ) -> crate::scripting::ast::ComponentExpression {
                ($body)(self)
            }
        }
    };
}
component!(
    DestroyableComponent,
    "destroyable",
    |s: &DestroyableComponent| {
        use super::ce_helpers::{CeBuilder, ce, nums};
        ce("Destroyable")
            .with_call("threshold", nums([s.threshold as f64]))
            .with_call("cell_size", nums([s.cell_size as f64]))
            .with_call("chunk_size", nums([s.chunk_size as f64]))
    }
);
component!(DamageComponent, "damage", |s: &DamageComponent| {
    super::ce_helpers::ce_call(
        "Damage",
        "at",
        super::ce_helpers::nums(
            s.position
                .into_iter()
                .chain([s.radius, s.amount])
                .map(f64::from),
        ),
    )
});
component!(
    CutGeometryComponent,
    "cut_geometry",
    |_: &CutGeometryComponent| { super::ce_helpers::ce("RuntimeCutGeometry") }
);

/// Runtime suppression keeps the original zone's authored enabled state intact.
#[derive(Debug, Clone, Copy)]
pub struct DestroyedSourceComponent;
component!(
    DestroyedSourceComponent,
    "destroyed_source",
    |_: &DestroyedSourceComponent| { super::ce_helpers::ce("RuntimeDestroyedSource") }
);
pub(crate) fn zone_suppressed(
    world: &crate::engine::ecs::World,
    id: crate::engine::ecs::ComponentId,
) -> bool {
    world.children_of(id).iter().any(|&child| {
        world
            .get_component_by_id_as::<DestroyedSourceComponent>(child)
            .is_some()
    })
}
