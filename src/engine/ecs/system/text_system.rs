use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    RenderableComponent, TextComponent, TextureComponent, TextureFilteringComponent,
    TransformComponent, UVComponent,
};
use crate::engine::graphics::VisualWorld;

#[derive(Debug, Default)]
pub struct TextSystem;

#[derive(Debug, Clone, Copy)]
pub struct SpawnedGlyph {
    pub transform: ComponentId,
    pub renderable: ComponentId,
    pub uv: ComponentId,
    pub texture: ComponentId,
}

impl TextSystem {
    pub fn register_text(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) -> Vec<SpawnedGlyph> {
        let Some(text_comp) = world.get_component_by_id_as::<TextComponent>(component) else {
            return Vec::new();
        };
        if text_comp.is_built() {
            return Vec::new();
        }

        let text = text_comp.text.clone();
        let wrap_at = text_comp.wrap_at;

        // If the TextComponent has an immediate TextureFilteringComponent child,
        // propagate it to all glyph renderables we spawn.
        let inherited_filtering = world
            .children_of(component)
            .iter()
            .find_map(|&ch| {
                world
                    .get_component_by_id_as::<TextureFilteringComponent>(ch)
                    .map(|c| c.filtering)
            });

        // Debug instrumentation: trace exactly what glyph subtrees get spawned.
        // (logger is currently a placeholder, so use stdout.)
        let debug_logs = std::env::var("LITTLE_CAT_TEXT_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let total_chars = text.chars().count();


        // Mark built immediately to avoid re-entrancy/double-build.
        if let Some(text_comp) = world.get_component_by_id_as_mut::<TextComponent>(component) {
            text_comp.mark_built();
        }

        let mut spawned = Vec::new();

        let mut col: usize = 0;
        let mut row: usize = 0;
        let mut line_count: usize = 0;

        let mut spawned_glyphs: usize = 0;

        for ch in text.chars() {
            if ch == '\n' {
                
                row += 1;
                col = 0;
                line_count = 0;
                continue;
            }

            // Huge perf win for code/text: don't spawn quads for whitespace.
            // Still advance the cursor so words separate visually.
            if ch == ' ' {
                col += 1;
                line_count += 1;
                continue;
            }
            if ch == '\t' {
                // Tab width: 4 spaces.
                col += 4;
                line_count += 4;
                continue;
            }

            if wrap_at > 0 && line_count >= wrap_at {
                
                row += 1;
                col = 0;
                line_count = 0;
            }

            let t = TransformComponent::new().with_position(col as f32, -(row as f32), 0.0);
            let t_id = world.add_component(t);
            let _ = world.add_child(component, t_id);

            let r_id = world.add_component(RenderableComponent::square());
            let _ = world.add_child(t_id, r_id);

            let uvs = uvs_for_glyph(ch);
            let uv_id = world.add_component(UVComponent { uvs });
            let _ = world.add_child(r_id, uv_id);

            let tex_id = world.add_component(TextureComponent::with_uri("assets/textures/font.dds"));
            let _ = world.add_child(r_id, tex_id);

            if let Some(filtering) = inherited_filtering {
                let f_id = world.add_component(TextureFilteringComponent::new(filtering));
                let _ = world.add_child(r_id, f_id);
            }

            spawned_glyphs += 1;

            spawned.push(SpawnedGlyph {
                transform: t_id,
                renderable: r_id,
                uv: uv_id,
                texture: tex_id,
            });

            col += 1;
            line_count += 1;
        }

        if debug_logs {
            println!(
                "[TextSystem] done: text_comp={:?} spawned_glyphs={} (total_chars={})",
                component, spawned_glyphs, total_chars
            );
        }

        spawned
    }
}

impl crate::engine::ecs::system::System for TextSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &crate::engine::user_input::InputState,
        _dt_sec: f32,
    ) {
        // Text expansion currently happens via registration.
    }
}

fn glyph_index(mut ch: char) -> usize {
    if ch.is_ascii_uppercase() {
        ch = ch.to_ascii_lowercase();
    }

    match ch {
        // a..h
        'a' => 0,
        'b' => 1,
        'c' => 2,
        'd' => 3,
        'e' => 4,
        'f' => 5,
        'g' => 6,
        'h' => 7,

        // i..p
        'i' => 8,
        'j' => 9,
        'k' => 10,
        'l' => 11,
        'm' => 12,
        'n' => 13,
        'o' => 14,
        'p' => 15,

        // q..x
        'q' => 16,
        'r' => 17,
        's' => 18,
        't' => 19,
        'u' => 20,
        'v' => 21,
        'w' => 22,
        'x' => 23,

        // y z 0 1 2 3 4 5
        'y' => 24,
        'z' => 25,
        '0' => 26,
        '1' => 27,
        '2' => 28,
        '3' => 29,
        '4' => 30,
        '5' => 31,

        // 6 7 8 9 0 ( ) ?
        '6' => 32,
        '7' => 33,
        '8' => 34,
        '9' => 35,
        '(' => 37,
        ')' => 38,
        '?' => 39,

        // [ ] { } < > _ #
        '[' => 40,
        ']' => 41,
        '{' => 42,
        '}' => 43,
        '<' => 44,
        '>' => 45,
        '_' => 46,
        '#' => 47,

        // % + - * / \ = .
        '%' => 48,
        '+' => 49,
        '-' => 50,
        '*' => 51,
        '/' => 52,
        '\\' => 53,
        '=' => 54,
        '.' => 55,

        // , : ; ' " ! @ &
        ',' => 56,
        ':' => 57,
        ';' => 58,
        '\'' => 59,
        '"' => 60,
        '!' => 61,
        '@' => 62,
        '&' => 63,

        // Fallback
        _ => 39, // '?'
    }
}

fn uvs_for_glyph(ch: char) -> Vec<[f32; 2]> {
    const COLS: f32 = 8.0;
    const ROWS: f32 = 8.0;

    let idx = glyph_index(ch);
    let row = (idx / 8) as f32;
    let col = (idx % 8) as f32;

    let u0 = col / COLS;
    let u1 = (col + 1.0) / COLS;

    // Atlas convention for `assets/textures/font.dds`:
    // - Row 0 is the TOP row of the image.
    // - The renderer/texture sampling treats v=0 as TOP and v=1 as BOTTOM.
    // So we do *not* flip V here.
    let v0 = row / ROWS;
    let v1 = (row + 1.0) / ROWS;

    // Quad vertex order from MeshFactory::quad_2d():
    // 0 bottom-left, 1 bottom-right, 2 top-right, 3 top-left
    vec![[u0, v0], [u1, v0], [u1, v1], [u0, v1]]
}
