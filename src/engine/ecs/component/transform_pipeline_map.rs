use super::Component;

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformMapTranslationComponent;

impl TransformMapTranslationComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformMapTranslationComponent {
    fn name(&self) -> &'static str {
        "transform_map_translation"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformMapRotationComponent;

impl TransformMapRotationComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformMapRotationComponent {
    fn name(&self) -> &'static str {
        "transform_map_rotation"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformMapScaleComponent;

impl TransformMapScaleComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformMapScaleComponent {
    fn name(&self) -> &'static str {
        "transform_map_scale"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
