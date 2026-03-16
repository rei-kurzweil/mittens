use super::Component;

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformPipelineComponent;

impl TransformPipelineComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformPipelineComponent {
    fn name(&self) -> &'static str {
        "transform_pipeline"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformForkTRSComponent;

impl TransformForkTRSComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformForkTRSComponent {
    fn name(&self) -> &'static str {
        "transform_fork_trs"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformMergeTRSComponent;

impl TransformMergeTRSComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformMergeTRSComponent {
    fn name(&self) -> &'static str {
        "transform_merge_trs"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformPipelineOutputComponent;

impl TransformPipelineOutputComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformPipelineOutputComponent {
    fn name(&self) -> &'static str {
        "transform_pipeline_output"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransformDropComponent;

impl TransformDropComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for TransformDropComponent {
    fn name(&self) -> &'static str {
        "transform_drop"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
