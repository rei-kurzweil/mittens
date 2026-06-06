use crate::engine::ecs::component::Component;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPayloadComponent {
    pub asset_key: String,
    pub title: String,
}

impl AssetPayloadComponent {
    pub fn new(asset_key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            asset_key: asset_key.into(),
            title: title.into(),
        }
    }
}

impl Component for AssetPayloadComponent {
    fn set_id(&mut self, _id: crate::engine::ecs::ComponentId) {}

    fn name(&self) -> &'static str {
        "asset_payload"
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
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call(
            "AssetPayload",
            "new",
            vec![s(&self.asset_key), s(&self.title)],
        )
    }
}
