use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
    InputComponent, InputTransformModeComponent, RenderableComponent, TextComponent,
    TextureFilteringComponent, TransformComponent,
};

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Deserialize)]
struct DependencyGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Deserialize)]
struct GraphNode {
    id: String,

    #[serde(default)]
    kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GraphEdge {
    from: String,
    to: String,
    via: String,
}

fn main() {
    utils::logger::init();

    const WRAP_AT: usize = 12;
    const WORD_WRAP_TOKENS: [&str; 1] = ["::"];
    // 180° yaw around +Y: quat(x,y,z,w) = (0, 1, 0, 0)
    const TEXT_YAW_180: [f32; 4] = [0.0, 1.0, 0.0, 0.0];

    // Push graphics further away from the camera along -Z.
    const GRAPHICS_Z_PUSH_WORLD: f32 = -6.0;

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Dark blue background.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.02, 0.03, 0.08, 1.0));
    universe.add(background);

    // Ambient so text/lines are readable without placing explicit lights.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.75, 0.75, 0.85));
    universe.add(ambient);

    // I { T { C3D } }
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(3.0));

    // Optional: match other examples (WASD + mouse, forward -Z).
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let cam_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 10.0));
    let _ = universe.attach(input, cam_transform);

    let camera = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(cam_transform, camera);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, cam_transform);

    universe.add(input);

    // Load graph.
    let json = std::fs::read_to_string("dependency-graph.json")
        .expect("failed to read dependency-graph.json");
    let graph: DependencyGraph =
        serde_json::from_str(&json).expect("failed to parse dependency-graph.json");

    // Place the whole graph in front of the camera (camera starts at +Z looking toward -Z).
    let center_z = -10.0f32;

    fn inferred_kind(node: &GraphNode) -> &'static str {
        if node.id.starts_with("engine::") {
            "engine"
        } else if node.id.starts_with("ecs::system::") {
            "ecs_system"
        } else if node.id.starts_with("ecs::") {
            "ecs_core"
        } else if node.id.starts_with("graphics::") {
            "graphics"
        } else if node.id.starts_with("utils::") {
            "utils"
        } else {
            "unknown"
        }
    }

    fn y_for_kind(kind: &str) -> f32 {
        match kind {
            "engine" => 3.0,
            "ecs_core" => 1.5,
            // Put graphics between systems and engine/world-ish stuff.
            "graphics" => 0.75,
            // Push systems down further (roughly 2x the previous gap).
            "ecs_system" => -1.5,
            "utils" => -3.0,
            _ => 0.0,
        }
    }

    // New layout for debugging: each layer (node kind) becomes its own X/Z grid.
    // We'll treat each TextComponent as a single-line string where each glyph advances by 1.0
    // in local-space X, then scaled by `text_scale`.
    let text_scale = 0.10f32;
    let char_advance_world = text_scale;
    let col_gap_world = 0.25f32;
    let col_inner_pad_world = 0.40f32;
    // Increase this to push rows further apart along Z.
    let row_gap_world = 3.20f32;

    // Per-layer spacing tweaks: keep most layers compact, but give systems more breathing room.
    let systems_col_gap_mult = 2.5f32;
    let systems_row_gap_mult = 1.75f32;

    // Deterministic ordering: group by kind, then sort ids within each kind.
    let mut layers: BTreeMap<String, Vec<&GraphNode>> = BTreeMap::new();
    for node in &graph.nodes {
        let kind = node
            .kind
            .clone()
            .unwrap_or_else(|| inferred_kind(node).to_string());
        layers.entry(kind).or_default().push(node);
    }
    for nodes in layers.values_mut() {
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
    }

    let mut positions: HashMap<String, [f32; 3]> = HashMap::new();
    let mut kinds_by_id: HashMap<String, String> = HashMap::new();

    for (kind, nodes) in layers {
        let n = nodes.len().max(1);
        let side = (n as f32).sqrt().ceil() as usize;
        let y = y_for_kind(&kind);
        let z_push_kind = if kind == "graphics" {
            GRAPHICS_Z_PUSH_WORLD
        } else {
            0.0
        };

        let col_gap_world_kind = if kind == "ecs_system" {
            col_gap_world * systems_col_gap_mult
        } else {
            col_gap_world
        };
        let row_gap_world_kind = if kind == "ecs_system" {
            row_gap_world * systems_row_gap_mult
        } else {
            row_gap_world
        };
        let row_height_world_kind = (1.0 * text_scale) + row_gap_world_kind;

        // Compute per-column max width based on the text length that will be rendered.
        // This gives us a simple table layout where columns widen to fit the longest string.
        let mut column_sizes: Vec<f32> = vec![0.0; side];
        for (i, node) in nodes.iter().enumerate() {
            let col = i % side;
            let chars_total = node.id.chars().count().max(1);
            let chars_per_line = chars_total.min(WRAP_AT) as f32;
            let width_world = (chars_per_line * char_advance_world) + col_inner_pad_world;
            if width_world > column_sizes[col] {
                column_sizes[col] = width_world;
            }
        }

        // Column start positions are cumulative widths.
        let mut col_starts: Vec<f32> = vec![0.0; side];
        for c in 1..side {
            col_starts[c] = col_starts[c - 1] + column_sizes[c - 1] + col_gap_world_kind;
        }
        let total_width_world =
            col_starts.last().copied().unwrap_or(0.0) + column_sizes.last().copied().unwrap_or(0.0);
        let x_offset = -0.5 * total_width_world;

        // Center rows around `center_z` as well.
        let rows = (n + side - 1) / side;
        let total_depth_world = (rows.saturating_sub(1) as f32) * row_height_world_kind;
        let z_offset = -0.5 * total_depth_world;

        for (i, node) in nodes.iter().enumerate() {
            let row = (i / side) as f32;
            let col = i % side;

            // Left-aligned table cell. (Text grows +X in local space.)
            let x = x_offset + col_starts[col];
            let z = center_z + z_offset + (row * row_height_world_kind) + z_push_kind;

            positions.insert(node.id.clone(), [x, y, z]);
            kinds_by_id.insert(node.id.clone(), kind.clone());

            // T { TXT { node.id } }
            let node_t = universe.world.add_component(
                TransformComponent::new()
                    .with_position(x, y, z)
                    //.with_rotation_quat(TEXT_YAW_180)
                    // Scale down text (glyphs are in whole units).
                    .with_scale(text_scale, text_scale, text_scale),
            );

            let node_txt = universe
                .world
                .add_component(TextComponent::with_word_wrap_tokens(
                    node.id.clone(),
                    WRAP_AT,
                    WORD_WRAP_TOKENS,
                ));
            let _ = universe.attach(node_t, node_txt);

            // Make text crispy/pixel-ish while debugging.
            // TextSystem looks for an immediate TextureFilteringComponent child.
            let node_txt_filter = universe
                .world
                .add_component(TextureFilteringComponent::nearest());
            let _ = universe.attach(node_txt, node_txt_filter);

            universe.add(node_t);
        }
    }

    // Re-enable edges: render as thin colored boxes with a label. Use quaternions to avoid
    // Euler-angle pitfalls.
    fn quat_from_unit_x_to_dir(dir: [f32; 3]) -> [f32; 4] {
        // Returns a quaternion (xyzw) rotating +X ([1,0,0]) to `dir`.
        let [dx, dy, dz] = dir;
        let dot = dx; // dot([1,0,0], dir)

        // If vectors are nearly identical, identity rotation.
        if dot > 0.999_999 {
            return [0.0, 0.0, 0.0, 1.0];
        }

        // If vectors are nearly opposite, rotate 180° around an axis orthogonal to +X.
        if dot < -0.999_999 {
            // Pick an arbitrary orthogonal axis; +Y works unless dir is +/-Y, but in this
            // opposite-to-+X case, dy/dz should be ~0 anyway.
            return [0.0, 1.0, 0.0, 0.0];
        }

        // cross([1,0,0], dir) = [0, -dz, dy]
        let qx = 0.0;
        let qy = -dz;
        let qz = dy;
        let qw = 1.0 + dot;

        // Normalize.
        let len2 = qx * qx + qy * qy + qz * qz + qw * qw;
        let inv_len = if len2 > 0.0 { len2.sqrt().recip() } else { 1.0 };
        [qx * inv_len, qy * inv_len, qz * inv_len, qw * inv_len]
    }

    for edge in &graph.edges {
        let Some(&a) = positions.get(&edge.from) else {
            continue;
        };
        let Some(&b) = positions.get(&edge.to) else {
            continue;
        };

        let from_kind = kinds_by_id
            .get(&edge.from)
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let to_kind = kinds_by_id
            .get(&edge.to)
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        // Color scheme:
        // - systems: cyan
        // - graphics: light reddish
        // - other: green
        let is_systems_edge = from_kind == "ecs_system" || to_kind == "ecs_system";

        let (edge_color_r, edge_color_g, edge_color_b) = if is_systems_edge {
            (0.0, 1.0, 1.0)
        } else if from_kind == "graphics" || to_kind == "graphics" {
            (1.0, 0.45, 0.45)
        } else {
            (0.25, 1.0, 0.25)
        };

        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let dz = b[2] - a[2];
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len < 0.000_1 {
            continue;
        }

        let inv_len = 1.0 / len;
        let dir = [dx * inv_len, dy * inv_len, dz * inv_len];
        let q = quat_from_unit_x_to_dir(dir);

        let mid = [
            (a[0] + b[0]) * 0.5,
            (a[1] + b[1]) * 0.5,
            (a[2] + b[2]) * 0.5,
        ];

        // Root carries position + rotation only.
        let edge_root = universe.world.add_component(
            TransformComponent::new()
                .with_position(mid[0], mid[1], mid[2])
                .with_rotation_quat(q),
        );

        // Box transform: inherits rotation, owns non-uniform scale.
        let thickness_mult = if is_systems_edge { 0.5 } else { 1.0 };
        let box_t = universe
            .world
            // Make the edge box 3x as narrow.
            .register(TransformComponent::new().with_scale(
                len,
                (0.02 / 3.0) * thickness_mult,
                (0.06 / 3.0) * thickness_mult,
            ));
        let _ = universe.attach(edge_root, box_t);

        let edge_r = universe.world.add_component(RenderableComponent::cube());
        let edge_c = universe.world.add_component(ColorComponent::rgba(
            edge_color_r,
            edge_color_g,
            edge_color_b,
            1.0,
        ));
        let _ = universe.attach(box_t, edge_r);
        let _ = universe.attach(edge_r, edge_c);

        // Label transform: not scaled. Place slightly "above" the edge in local Y.
        let label_t = universe.world.add_component(
            TransformComponent::new()
                .with_position(0.0, 0.25, 0.15)
                .with_rotation_quat(TEXT_YAW_180)
                .with_scale(0.05, 0.05, 0.05),
        );
        let _ = universe.attach(edge_root, label_t);

        let label_txt = universe
            .world
            .add_component(TextComponent::with_word_wrap_tokens(
                edge.via.clone(),
                WRAP_AT,
                WORD_WRAP_TOKENS,
            ));
        let _ = universe.attach(label_t, label_txt);

        let label_filter = universe
            .world
            .add_component(TextureFilteringComponent::nearest());
        let _ = universe.attach(label_txt, label_filter);

        universe.add(edge_root);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
