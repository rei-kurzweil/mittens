//! Bridge between `World` topology and the shared `src/query/` evaluator.
//!
//! `WorldQueryAdapter` lets `QueryEvaluator` walk the live ECS using the same
//! adapter trait that the module CE-tree query uses. Engine subsystems that
//! need to look up components by name/type should go through this adapter
//! rather than rolling their own walks.

use crate::engine::ecs::{ComponentId, World};
use crate::query::{AttributeSelector, QueryTreeAdapter};

pub struct WorldQueryAdapter<'w> {
    world: &'w World,
}

impl<'w> WorldQueryAdapter<'w> {
    pub fn new(world: &'w World) -> Self {
        Self { world }
    }
}

impl<'w> QueryTreeAdapter for WorldQueryAdapter<'w> {
    type NodeId = ComponentId;

    fn children_of(&self, node: Self::NodeId) -> Vec<Self::NodeId> {
        self.world.children_of(node).to_vec()
    }

    fn matches_type(&self, node: Self::NodeId, type_name: &str) -> bool {
        self.world.component_name(node).map_or(false, |t| t == type_name)
    }

    fn matches_name(&self, node: Self::NodeId, name: &str) -> bool {
        self.world.component_label(node).map_or(false, |n| n == name)
    }

    fn matches_attribute(&self, node: Self::NodeId, attribute: &AttributeSelector) -> bool {
        if attribute.name == "name" {
            return attribute
                .value
                .as_deref()
                .map(|name| self.matches_name(node, name))
                .unwrap_or(false);
        }
        false
    }
}
