use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone)]
pub(crate) struct SignalPipeline {
    pub source_operator: ComponentId,
    pub ops: Vec<SignalPipelineOp>,
}

#[derive(Debug, Clone)]
pub(crate) enum SignalPipelineOp {
    RouteUpward(SignalRouteUpward),
}

#[derive(Debug, Clone)]
pub(crate) struct SignalRouteUpward {
    /// Which intent kind this routing applies to.
    ///
    /// Convention: "any" applies to all intents.
    pub intent_kind: String,

    /// Component type name (from `Component::name()`) at which traversal stops.
    pub parent_type: String,
}

impl SignalRouteUpward {
    pub fn applies_to_intent_kind(&self, kind_name: &str) -> bool {
        let want = self.intent_kind.as_str();
        want.is_empty() || want == "any" || want == kind_name
    }

    pub fn route(&self, world: &World, start: ComponentId) -> ComponentId {
        if self.parent_type.is_empty() {
            return start;
        }

        // "Ancestor" routing: do not match the start node itself.
        let mut cur = world.parent_of(start);
        while let Some(cid) = cur {
            let Some(node) = world.get_component_node(cid) else {
                break;
            };

            if node.component.name() == self.parent_type {
                return cid;
            }

            cur = world.parent_of(cid);
        }

        start
    }
}
