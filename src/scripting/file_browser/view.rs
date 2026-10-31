//! A fixed 5 x 4 native grid. Meshes are cached; refreshes reuse every tile.
use super::{
    Control, ControlComponent,
    model::{Model, PAGE_SIZE},
};
use crate::engine::ecs::component::style::{
    Display, FlexDirection, SizeDimension as D, TextAlign, VerticalAlign, WordWrapMode,
};
use crate::engine::{
    ecs::{
        ComponentId, IntentValue, SignalEmitter, World,
        component::{
            ColorComponent, Component, LayoutComponent, OptionComponent, RaycastableComponent,
            RenderableComponent, SelectionComponent, SelectionEntry, StyleComponent, TextComponent,
            TransformComponent,
        },
    },
    graphics::{
        RenderAssets,
        primitives::{CpuMeshHandle, MaterialHandle},
    },
};

pub struct Tile {
    pub owner: ComponentId,
    pub style: ComponentId,
    pub text: ComponentId,
    pub color: ComponentId,
}
pub struct View {
    pub selection: ComponentId,
    pub path: ComponentId,
    pub page: ComponentId,
    pub tiles: Vec<Tile>,
}
fn child<T: Component>(world: &mut World, parent: ComponentId, value: T) -> ComponentId {
    let id = world.add_component(value);
    world.add_child(parent, id).unwrap();
    id
}
fn box_node(
    world: &mut World,
    parent: ComponentId,
    name: &str,
    w: f32,
    h: f32,
) -> (ComponentId, ComponentId) {
    let node = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
    world.add_child(parent, node).unwrap();
    let mut style = StyleComponent::default();
    style.display = Some(Display::Block);
    style.width = D::GlyphUnits(w);
    style.height = D::GlyphUnits(h);
    let style = child(world, node, style);
    (node, style)
}
fn row(world: &mut World, parent: ComponentId, height: f32) -> ComponentId {
    let (node, style) = box_node(world, parent, "browser_row", 57.0, height);
    let style = world
        .get_component_by_id_as_mut::<StyleComponent>(style)
        .unwrap();
    style.display = Some(Display::Flex);
    style.flex_direction = FlexDirection::Row;
    style.column_gap = 0.6;
    node
}
fn button(
    world: &mut World,
    root: ComponentId,
    name: &str,
    label: &str,
    control: Control,
) -> ComponentId {
    let (node, style) = box_node(world, root, name, 3.5, 2.8);
    let style = world
        .get_component_by_id_as_mut::<StyleComponent>(style)
        .unwrap();
    style.background_color = Some([0.96, 0.73, 0.28, 1.0]);
    style.text_align = TextAlign::Center;
    child(world, node, RaycastableComponent::enabled());
    child(world, node, ControlComponent(control));
    child(world, node, TextComponent::new(label));
    node
}
fn polygon(
    world: &mut World,
    root: ComponentId,
    mesh: CpuMeshHandle,
    color: [f32; 4],
    z: f32,
) -> ComponentId {
    let node = child(
        world,
        root,
        TransformComponent::new().with_position(0.0, 0.0, z),
    );
    // The enclosing MMS shell supplies its generic custom material.
    let renderable = child(
        world,
        node,
        RenderableComponent::from_cpu_mesh_handle(mesh, MaterialHandle::TOON_MESH),
    );
    child(world, renderable, ColorComponent { rgba: color })
}
pub fn build(world: &mut World, assets: &mut RenderAssets) -> Result<(ComponentId, View), String> {
    let root = world.add_component_boxed_named("file_browser", Box::new(TransformComponent::new()));
    let mut style = StyleComponent::default();
    style.display = Some(Display::Flex);
    style.flex_direction = FlexDirection::Column;
    style.row_gap = 0.5;
    style.width = D::GlyphUnits(57.0);
    style.height = D::GlyphUnits(43.0);
    child(world, root, style);
    let mut selection = SelectionComponent::new();
    selection.visual_feedback = false;
    let selection = child(world, root, selection);
    let header = row(world, root, 3.0);
    let up = button(world, header, "browser_up", "", Control::Up);
    let arrow = assets.polygon_mesh(
        "file-browser/up-arrow-v1",
        &[
            [-0.18, -0.7],
            [0.18, -0.7],
            [0.18, 0.05],
            [0.65, 0.05],
            [0.0, 0.75],
            [-0.65, 0.05],
            [-0.18, 0.05],
        ],
    )?;
    polygon(world, up, arrow, [0.12, 0.08, 0.025, 1.0], 0.01);
    let (path_box, _) = box_node(world, header, "browser_path_bar", 52.0, 3.0);
    let path = child(world, path_box, TextComponent::new("Waiting for PC"));
    let page_mesh = assets.polygon_mesh(
        "file-browser/page-v1",
        &[
            [-1.45, -1.8],
            [1.45, -1.8],
            [1.45, 0.8],
            [0.45, 1.8],
            [-1.45, 1.8],
        ],
    )?;
    let shadow_mesh = assets.polygon_mesh(
        "file-browser/fold-shadow-v1",
        &[[0.35, 0.60], [1.45, 0.60], [0.35, 1.8]],
    )?;
    let fold_mesh = assets.polygon_mesh(
        "file-browser/fold-v1",
        &[[0.45, 0.8], [1.45, 0.8], [0.45, 1.8]],
    )?;
    let mut tiles = Vec::with_capacity(PAGE_SIZE);
    let mut grid_row = root;
    for index in 0..PAGE_SIZE {
        if index % 5 == 0 {
            grid_row = row(world, root, 8.3);
        }
        let (tile, style) = box_node(world, grid_row, &format!("browser_tile_{index}"), 10.8, 8.3);
        child(world, tile, RaycastableComponent::enabled());
        child(world, tile, ControlComponent(Control::Entry(index)));
        child(world, tile, OptionComponent::new());
        world
            .get_component_by_id_as_mut::<StyleComponent>(style)
            .unwrap()
            .background_color = Some([1.0, 0.97, 0.86, 1.0]);
        let (icon, icon_style) = box_node(world, tile, "browser_page_icon", 10.8, 4.7);
        let icon_style = world
            .get_component_by_id_as_mut::<StyleComponent>(icon_style)
            .unwrap();
        icon_style.text_align = TextAlign::Center;
        icon_style.vertical_align = VerticalAlign::Middle;
        let graphic = child(world, icon, TransformComponent::new());
        let color = polygon(world, graphic, page_mesh, [1.0, 0.89, 0.57, 1.0], 0.01);
        polygon(world, graphic, shadow_mesh, [0.64, 0.43, 0.12, 1.0], 0.02);
        polygon(world, graphic, fold_mesh, [1.0, 0.98, 0.84, 1.0], 0.03);
        let (title, title_style) = box_node(world, tile, "browser_filename_box", 10.8, 3.3);
        let style_ref = world
            .get_component_by_id_as_mut::<StyleComponent>(title_style)
            .unwrap();
        style_ref.font_size = D::GlyphUnits(1.0);
        style_ref.text_align = TextAlign::Center;
        style_ref.word_wrap = Some(WordWrapMode::BreakWord);
        let text = child(world, title, TextComponent::with_wrap("", 10));
        tiles.push(Tile {
            owner: tile,
            style,
            text,
            color,
        });
    }
    let footer = row(world, root, 2.8);
    button(world, footer, "browser_previous", "<", Control::Previous);
    button(world, footer, "browser_next", ">", Control::Next);
    let (page_box, _) = box_node(world, footer, "browser_page_bar", 48.0, 2.8);
    let page = child(world, page_box, TextComponent::new(""));
    Ok((
        root,
        View {
            selection,
            path,
            page,
            tiles,
        },
    ))
}
fn set_text(world: &World, emit: &mut dyn SignalEmitter, id: ComponentId, text: String) {
    if world
        .get_component_by_id_as::<TextComponent>(id)
        .is_some_and(|old| old.text != text)
    {
        emit.push_intent_now(
            id,
            IntentValue::SetText {
                component_id: id,
                text,
            },
        );
    }
}
pub fn refresh(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    root: ComponentId,
    model: &Model,
    view: &View,
) {
    set_text(world, emit, view.path, model.path.clone());
    let page = if model.error.is_empty() {
        format!(
            "{} items  /  rows {}-{} of {}",
            model.entries.len(),
            model.page + 1,
            (model.page + 4).min(model.entries.len().div_ceil(5)),
            model.entries.len().div_ceil(5).max(1)
        )
    } else {
        model.error.chars().take(75).collect()
    };
    set_text(world, emit, view.page, page);
    for (slot, tile) in view.tiles.iter().enumerate() {
        let entry = model.entries.get(model.page * 5 + slot);
        let style = world
            .get_component_by_id_as_mut::<StyleComponent>(tile.style)
            .unwrap();
        style.display = Some(if entry.is_some() {
            Display::Block
        } else {
            Display::None
        });
        if let Some(entry) = entry {
            world
                .get_component_by_id_as_mut::<StyleComponent>(tile.style)
                .unwrap()
                .background_color = Some(if model.selected_id == entry.id {
                [0.94, 0.8864, 0.69, 1.0]
            } else {
                [1.0, 0.97, 0.86, 1.0]
            });
            let label = wrapped_label(&entry.name, entry.kind == "directory");
            set_text(world, emit, tile.text, label);
            let rgba = if entry.kind == "directory" {
                [0.95, 0.65, 0.18, 1.0]
            } else {
                [1.0, 0.89, 0.57, 1.0]
            };
            emit.push_intent_now(
                tile.color,
                IntentValue::SetColor {
                    component_id: tile.color,
                    rgba,
                },
            );
        }
    }
    let selected = model
        .entries
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.id == model.selected_id)
        .and_then(|(index, _)| {
            index
                .checked_sub(model.page * 5)
                .filter(|&slot| slot < PAGE_SIZE)
                .map(|slot| SelectionEntry {
                    index: Some(index),
                    component: view.tiles[slot].owner,
                })
        });
    if let Some(selection) = world.get_component_by_id_as_mut::<SelectionComponent>(view.selection)
    {
        if let Some(entry) = selected {
            selection.select_entry(entry);
        } else {
            selection.clear();
        }
    }
    let mut ancestor = Some(root);
    while let Some(id) = ancestor {
        if let Some(layout) = world.get_component_by_id_as_mut::<LayoutComponent>(id) {
            layout.mark_dirty();
            break;
        }
        ancestor = world.parent_of(id);
    }
}

// Prefer filename separators, then split long words. The three-line tile title
// remains bounded while preserving the original name's visible characters.
fn wrapped_label(name: &str, directory: bool) -> String {
    let mut remaining: Vec<char> = name.chars().collect();
    if directory {
        remaining.push('/');
    }
    let mut result = String::new();
    for line in 0..3 {
        if remaining.is_empty() {
            break;
        }
        if line > 0 {
            result.push('\n');
        }
        if remaining.len() <= 10 {
            result.extend(remaining.drain(..));
            break;
        }
        if line == 2 {
            result.extend(remaining.drain(..7));
            result.push_str("...");
            break;
        }
        let split = remaining[..10]
            .iter()
            .rposition(|ch| matches!(ch, '-' | '_' | ' ' | '.'))
            .map(|index| index + 1)
            .filter(|&index| index >= 4)
            .unwrap_or(10);
        result.extend(remaining.drain(..split));
    }
    result
}
