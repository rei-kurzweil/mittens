use super::Component;
use crate::engine::ecs::system::triangle_collision::TriangleMesh;
use std::sync::Arc;

/// Opts sibling cube/box pairs, descendant section pieces, or a parent imported
/// rigid Renderable into runtime destruction. Retained CombineMesh batches
/// unpack on the first successful cut. Damage state is per instance and never
/// enters authored serialization. Imported collision is a separate opt-in.
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
    /// The push that comes with the hit (a rocket's, not a pistol's).
    pub blast: Option<Blast>,
}
/// A damage impact's push, out to `radius` from its centre (usually well
/// beyond the cut radius): debris flies off at up to `speed` m/s spinning at
/// up to `spin` rad/s, and Blastable bodies take the same push scaled by their
/// mass. Strength falls off linearly to zero at `radius`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blast {
    pub radius: f32,
    pub speed: f32,
    pub spin: f32,
}
impl Blast {
    pub fn new(radius: f32, speed: f32, spin: f32) -> Result<Self, String> {
        if !(radius.is_finite() && speed.is_finite() && spin.is_finite())
            || radius <= 0.
            || speed < 0.
            || spin < 0.
        {
            return Err("blast(radius, speed, spin) needs a positive radius and non-negative speed and spin".into());
        }
        Ok(Self { radius, speed, spin })
    }
    /// Push strength at `distance` from the centre: 1 at the centre, 0 at
    /// and beyond `radius`.
    pub fn falloff(&self, distance: f32) -> f32 {
        (1. - distance / self.radius).clamp(0., 1.)
    }
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
    use super::ce_helpers::{CeBuilder, ce_call, nums};
    let expression = ce_call(
        "Damage",
        "at",
        nums(
            s.position
                .into_iter()
                .chain([s.radius, s.amount])
                .map(f64::from),
        ),
    );
    match s.blast {
        Some(b) => expression.with_call("blast", nums([b.radius, b.speed, b.spin].map(f64::from))),
        None => expression,
    }
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

/// Identifies derived grid frames so retained batches only bake authored sources.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DestructionCellComponent;
component!(
    DestructionCellComponent,
    "destruction_cell",
    |_: &DestructionCellComponent| { super::ce_helpers::ce("RuntimeDestructionCell") }
);
