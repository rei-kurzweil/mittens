use crate::engine::ecs::component::{
    ColorComponent, DataComponent, DataValue, Display, EdgeInsets, OptionComponent, Overflow,
    RaycastableComponent, SizeDimension, StyleComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::{ComponentId, World};

pub struct PanelUiRowSpec<'a> {
    pub row_name: &'a str,
    pub payload_name: &'a str,
    pub target_component: Option<ComponentId>,
    pub label: &'a str,
    pub row_kind_label: &'a str,
    pub interactive: bool,
    pub background_rgba: [f32; 4],
    pub text_rgba: [f32; 4],
    pub font_size_gu: Option<f32>,
    pub spacer_height_gu: Option<f32>,
}

pub fn spawn_panel_ui_row_tree(world: &mut World, spec: PanelUiRowSpec<'_>) -> ComponentId {
    let row_root =
        world.add_component_boxed_named(spec.row_name, Box::new(TransformComponent::new()));

    if let Some(height_gu) = spec.spacer_height_gu {
        let style = world.add_component_boxed_named(
            format!("{}_style", spec.row_name),
            Box::new({
                let mut style = StyleComponent::new();
                style.display = Some(Display::Block);
                style.width = SizeDimension::Percent(100.0);
                style.height = SizeDimension::GlyphUnits(height_gu);
                style.overflow = Overflow::Visible;
                style
            }),
        );
        let _ = world.add_child(row_root, style);
        return row_root;
    }

    let option_root = if spec.interactive {
        let option = world.add_component_boxed_named(
            format!("{}_option", spec.row_name),
            Box::new(OptionComponent::new()),
        );
        let _ = world.add_child(row_root, option);
        Some(option)
    } else {
        None
    };
    if spec.interactive {
        let raycastable = world.add_component_boxed_named(
            format!("{}_raycastable", spec.row_name),
            Box::new(RaycastableComponent::click_only()),
        );
        let _ = world.add_child(row_root, raycastable);
    }

    let mut payload_data = DataComponent::new()
        .with_entry("row_name", DataValue::Text(spec.row_name.to_string()))
        .with_entry("row_kind", DataValue::Text(spec.row_kind_label.to_string()))
        .with_entry("label", DataValue::Text(spec.label.to_string()))
        .with_entry("interactive", DataValue::Bool(spec.interactive));
    if let Some(target_component) = spec.target_component {
        payload_data.insert("target_component", DataValue::Component(target_component));
    }
    let payload = world.add_component_boxed_named(spec.payload_name, Box::new(payload_data));
    let _ = world.add_child(option_root.unwrap_or(row_root), payload);

    let style = world.add_component_boxed_named(
        format!("{}_style", spec.row_name),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.margin = EdgeInsets::axes(0.25, 0.20);
            style.padding = EdgeInsets::axes(0.55, 0.45);
            if let Some(font_size_gu) = spec.font_size_gu {
                style.font_size = SizeDimension::GlyphUnits(font_size_gu);
            }
            style.background_color = Some(spec.background_rgba);
            style.background_z = Some(0.001);
            style.color = Some(spec.text_rgba);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(row_root, style);

    let text_root = world.add_component_boxed_named(
        format!("{}_text_root", spec.row_name),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
    );
    let text = world.add_component_boxed_named(
        format!("{}_text", spec.row_name),
        Box::new(TextComponent::new(spec.label.to_string())),
    );
    let text_color = world.add_component_boxed_named(
        format!("{}_text_color", spec.row_name),
        Box::new(ColorComponent::rgba(
            spec.text_rgba[0],
            spec.text_rgba[1],
            spec.text_rgba[2],
            spec.text_rgba[3],
        )),
    );

    let _ = world.add_child(row_root, text_root);
    let _ = world.add_child(text_root, text);
    let _ = world.add_child(text, text_color);

    row_root
}

pub fn spawn_block_container(world: &mut World, name: &str) -> ComponentId {
    let root = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
    let style = world.add_component_boxed_named(
        format!("{name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(root, style);
    root
}

pub fn spawn_panel_ui_section_header_tree(
    world: &mut World,
    row_name: &str,
    label: &str,
) -> ComponentId {
    let row_root = world.add_component_boxed_named(row_name, Box::new(TransformComponent::new()));

    let style = world.add_component_boxed_named(
        format!("{row_name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.margin = EdgeInsets::axes(0.0, 0.35);
            style.padding = EdgeInsets::axes(0.4, 0.2);
            style.background_color = Some([0.16, 0.20, 0.18, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.95, 0.98, 0.92, 1.0]);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(row_root, style);

    let text_root = world.add_component_boxed_named(
        format!("{row_name}_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
    );
    let text = world.add_component_boxed_named(
        format!("{row_name}_text"),
        Box::new(TextComponent::new(label.to_string())),
    );
    let text_color = world.add_component_boxed_named(
        format!("{row_name}_text_color"),
        Box::new(ColorComponent::rgba(0.95, 0.98, 0.92, 1.0)),
    );

    let _ = world.add_child(row_root, text_root);
    let _ = world.add_child(text_root, text);
    let _ = world.add_child(text, text_color);

    row_root
}
