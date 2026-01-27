use crate::engine::ecs;

use super::color;

/// Format a single component as an `ls`-style summary line.
///
/// Returns `None` if the component no longer exists.
pub fn format_ls_line(world: &ecs::World, index: usize, cid: ecs::ComponentId) -> Option<String> {
    let node = world.get_component_node(cid)?;

    let type_name = node.component.name();
    let base_rgb: Option<(u8, u8, u8)> = match type_name {
        "renderable" => Some((255, 0, 200)),
        "input" => Some((40, 255, 10)),
        "camera3d" | "camera2d" => Some((0, 160, 255)),
        _ => None,
    };

    if let Some(base_rgb) = base_rgb {
        let type_rgb = color::scale_rgb(base_rgb, 0.6);
        let base = color::fg_rgb(base_rgb.0, base_rgb.1, base_rgb.2);
        let type_color = color::fg_rgb(type_rgb.0, type_rgb.1, type_rgb.2);

        Some(format!(
            "{base}🐈 {index}: {name}  type={type_color}{type_name}{base}  guid={guid}{reset}",
            name = node.name,
            guid = node.guid,
            reset = color::RESET
        ))
    } else {
        Some(format!(
            "🐈 {}: {}  type={}  guid={}",
            index, node.name, type_name, node.guid
        ))
    }
}
