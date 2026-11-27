//! An authored waypoint graph queried by scripts through `PathFindingSystem`.
use super::Component;
use crate::engine::ecs::system::path_finding_system::PathGraph;
use crate::engine::ecs::{ComponentId, SignalEmitter};

#[derive(Debug)]
pub struct PathGraphComponent {
    pub graph: PathGraph,
}

impl Component for PathGraphComponent {
    fn name(&self) -> &'static str {
        "path_graph"
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn init(&mut self, _: &mut dyn SignalEmitter, _: ComponentId) {}
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::{array, ce_call, nums};
        use crate::scripting::ast::{Expression, Ident, TableFieldValue};
        let field = |name: &str, value| TableFieldValue {
            name: Ident(name.into()),
            value,
        };
        let list = |rows: Vec<Vec<f64>>| array(rows.into_iter().map(|row| array(nums(row))).collect());
        let table = Expression::Table(vec![
            field(
                "nodes",
                list(self.graph.nodes().iter().map(|node| node.to_vec()).collect()),
            ),
            field(
                "obstacles",
                list(self.graph.obstacles().iter().map(|rect| rect.to_vec()).collect()),
            ),
            field("bounds", array(nums(self.graph.bounds()))),
        ]);
        ce_call("PathGraph", "new", vec![table])
    }
}
