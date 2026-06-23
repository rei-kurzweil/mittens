use super::{Component, ComponentRef, QueryRootMode, resolve_component_ref};
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, Default)]
pub struct TransformParentComponent {
    pub target_source: Option<ComponentRef>,
    pub root_source: Option<ComponentRef>,
}

impl TransformParentComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_target_source(mut self, src: ComponentRef) -> Self {
        self.target_source = Some(src);
        self
    }

    pub fn with_root_source(mut self, src: ComponentRef) -> Self {
        self.root_source = Some(src);
        self
    }

    pub fn resolve_root_component(&self, world: &World) -> Option<ComponentId> {
        let src = self.root_source.as_ref()?;
        resolve_component_ref(world, src, None, QueryRootMode::WorldRoot)
    }

    pub fn resolve_target_component(&self, world: &World) -> Option<ComponentId> {
        let src = self.target_source.as_ref()?;
        let scope_root = match src {
            ComponentRef::Guid(_) => None,
            ComponentRef::Query(_) => self.resolve_root_component(world),
        };
        Self::resolve_component_ref(world, src, scope_root)
    }

    fn resolve_component_ref(
        world: &World,
        src: &ComponentRef,
        scope_root: Option<ComponentId>,
    ) -> Option<ComponentId> {
        match src {
            ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
            ComponentRef::Query(selector) => {
                if let Some(root) = scope_root {
                    return resolve_component_ref(
                        world,
                        &ComponentRef::Query(selector.clone()),
                        Some(root),
                        QueryRootMode::SelfSubtree,
                    );
                }
                resolve_component_ref(
                    world,
                    &ComponentRef::Query(selector.clone()),
                    None,
                    QueryRootMode::WorldRoot,
                )
            }
        }
    }
}

impl Component for TransformParentComponent {
    fn name(&self) -> &'static str {
        "transform_parent"
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
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::{CeBuilder, ce, ce_call};
        use crate::meow_meow::ast::Expression;

        fn target_expr(t: &ComponentRef) -> Expression {
            match t {
                ComponentRef::Guid(u) => Expression::String(format!("@uuid:{u}")),
                ComponentRef::Query(s) => Expression::String(s.clone()),
            }
        }

        let mut ce = match &self.target_source {
            Some(src) => ce_call("TransformParent", "target", vec![target_expr(src)]),
            None => ce("TransformParent"),
        };
        if let Some(src) = &self.root_source {
            ce = ce.with_call("root", vec![target_expr(src)]);
        }
        ce
    }
}
