use crate::engine::ecs::{ComponentId, World, component::Component};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataValue {
    Text(String),
    Integer(i64),
    Bool(bool),
    Component(ComponentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataEntry {
    pub key: String,
    pub value: DataValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataComponent {
    entries: Vec<DataEntry>,
}

impl DataComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entry(mut self, key: impl Into<String>, value: DataValue) -> Self {
        self.insert(key, value);
        self
    }

    pub fn insert(&mut self, key: impl Into<String>, value: DataValue) {
        let key = key.into();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.value = value;
            return;
        }
        self.entries.push(DataEntry { key, value });
    }

    pub fn get(&self, key: &str) -> Option<&DataValue> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }

    pub fn get_component(&self, key: &str) -> Option<ComponentId> {
        match self.get(key)? {
            DataValue::Component(component_id) => Some(*component_id),
            _ => None,
        }
    }

    pub fn entries(&self) -> &[DataEntry] {
        &self.entries
    }

    pub fn entry(&self, index: usize) -> Option<&DataEntry> {
        self.entries.get(index)
    }
}

impl Component for DataComponent {
    fn set_id(&mut self, _id: ComponentId) {}

    fn name(&self) -> &'static str {
        "data"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self, _world: &World) -> crate::meow_meow::ast::ComponentExpression {
        crate::engine::ecs::component::ce_helpers::ce("Data")
    }
}
