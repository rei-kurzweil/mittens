use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    EmissiveComponent, RaycastableComponent, RenderableComponent, TextComponent,
    TextureComponent, TextureFilteringComponent, TransformComponent, UVComponent,
};
use crate::engine::graphics::VisualWorld;

#[derive(Debug, Default)]
pub struct TextSystem;

#[derive(Debug, Clone, Copy)]
struct WordWrapState {
    col: usize,
    row: usize,
    line_count: usize,
    last_wrap_allowed: bool,
    wrap_at: usize,
    word_wrap: bool,
}

impl WordWrapState {
    const TAB_WIDTH: usize = 4;

    fn new(wrap_at: usize, word_wrap: bool) -> Self {
        Self {
            col: 0,
            row: 0,
            line_count: 0,
            last_wrap_allowed: false,
            wrap_at,
            word_wrap,
        }
    }

    fn newline(&mut self) {
        self.row += 1;
        self.col = 0;
        self.line_count = 0;
        self.last_wrap_allowed = false;
    }

    fn apply_wrap_if_needed(&mut self) {
        if self.wrap_at == 0 || self.line_count < self.wrap_at {
            return;
        }

        // Word-wrap mode: only wrap if we previously encountered a wrap token.
        // Otherwise keep going to avoid breaking words.
        let should_wrap = if self.word_wrap {
            self.last_wrap_allowed && self.col > 0
        } else {
            true
        };

        if should_wrap {
            self.row += 1;
            self.col = 0;
            self.line_count = 0;
        }
    }

    fn cursor_pos(&self) -> (f32, f32) {
        (self.col as f32, -(self.row as f32))
    }

    fn advance_space(&mut self, i: usize, wrap_allowed_after: &[bool]) {
        self.col += 1;
        self.line_count += 1;
        self.last_wrap_allowed = wrap_allowed_after.get(i).copied().unwrap_or(false);
    }

    fn advance_tab(&mut self, i: usize, wrap_allowed_after: &[bool]) {
        self.col += Self::TAB_WIDTH;
        self.line_count += Self::TAB_WIDTH;
        self.last_wrap_allowed = wrap_allowed_after.get(i).copied().unwrap_or(false);
    }

    fn advance_glyph(&mut self, i: usize, wrap_allowed_after: &[bool]) {
        self.col += 1;
        self.line_count += 1;
        self.last_wrap_allowed = wrap_allowed_after.get(i).copied().unwrap_or(false);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpawnedGlyph {
    pub transform: ComponentId,
    pub renderable: ComponentId,
    pub uv: ComponentId,
    pub texture: ComponentId,
}

impl TextSystem {
    fn handle_word_wrap_for(
        ch: char,
        i: usize,
        wrap_allowed_after: &[bool],
        state: &mut WordWrapState,
    ) -> bool {
        if ch == '\n' {
            state.newline();
            return false;
        }

        state.apply_wrap_if_needed();

        // Huge perf win for code/text: don't spawn quads for whitespace.
        // Still advance the cursor so words separate visually.
        if ch == ' ' {
            state.advance_space(i, wrap_allowed_after);
            return false;
        }

        if ch == '\t' {
            state.advance_tab(i, wrap_allowed_after);
            return false;
        }

        true
    }

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
        let word_wrap = text_comp.word_wrap;
        let word_wrap_tokens = text_comp.word_wrap_tokens.clone();

        // Allow overriding the font atlas by attaching an immediate TextureComponent to the
        // TextComponent root.
        let inherited_font_texture_uri = world
            .children_of(component)
            .iter()
            .find_map(|&ch| {
                world
                    .get_component_by_id_as::<TextureComponent>(ch)
                    .and_then(|t| t.uri().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "assets/textures/font_system.dds".to_string());

        // If the TextComponent has an immediate TextureFilteringComponent child,
        // propagate it to all glyph renderables we spawn.
        let inherited_filtering = world.children_of(component).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<TextureFilteringComponent>(ch)
                .map(|c| c.filtering)
        });

        // Also allow styling at the TextComponent root: immediate Emissive children.
        // (Color is now inherited by renderables from ancestors; no per-glyph ColorComponent needed.)
        let inherited_emissive = world.children_of(component).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<EmissiveComponent>(ch)
                .map(|e| e.enabled)
        });

        // Raycasting is explicit opt-in. For text, allow toggling at the TextComponent root by
        // attaching an immediate RaycastableComponent child; this is propagated to all glyphs.
        let inherited_raycastable = world.children_of(component).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<RaycastableComponent>(ch)
                .map(|r| r.enable)
        });

        // Mark built immediately to avoid re-entrancy/double-build.
        if let Some(text_comp) = world.get_component_by_id_as_mut::<TextComponent>(component) {
            text_comp.mark_built();
        }

        let mut spawned = Vec::new();

        let chars: Vec<char> = text.chars().collect();
        let wrap_allowed_after: Vec<bool> = compute_wrap_allowed_after(&chars, &word_wrap_tokens);

        let mut wrap_state = WordWrapState::new(wrap_at, word_wrap);

        for (i, ch) in chars.iter().copied().enumerate() {
            if !Self::handle_word_wrap_for(ch, i, &wrap_allowed_after, &mut wrap_state) {
                continue;
            }

            let (x, y) = wrap_state.cursor_pos();
            let t = TransformComponent::new().with_position(x, y, 0.0);
            let t_id = world.add_component(t);
            let _ = world.add_child(component, t_id);

            let r_id = world.add_component(RenderableComponent::square());
            let _ = world.add_child(t_id, r_id);

            if let Some(enable) = inherited_raycastable {
                let rc_id = world.add_component(RaycastableComponent::new(enable));
                let _ = world.add_child(r_id, rc_id);
            }

            let uvs = uvs_for_glyph(ch);
            let uv_id = world.add_component(UVComponent { uvs });
            let _ = world.add_child(r_id, uv_id);

            let tex_id = world.add_component(TextureComponent::with_uri(
                inherited_font_texture_uri.clone(),
            ));
            let _ = world.add_child(r_id, tex_id);

            if let Some(filtering) = inherited_filtering {
                let f_id = world.add_component(TextureFilteringComponent::new(filtering));
                let _ = world.add_child(r_id, f_id);
            }

            if let Some(enabled) = inherited_emissive {
                let e_id = world.add_component(EmissiveComponent { enabled });
                let _ = world.add_child(r_id, e_id);
            }

            spawned.push(SpawnedGlyph {
                transform: t_id,
                renderable: r_id,
                uv: uv_id,
                texture: tex_id,
            });

            wrap_state.advance_glyph(i, &wrap_allowed_after);
        }

        spawned
    }
}

fn compute_wrap_allowed_after(chars: &[char], tokens: &[String]) -> Vec<bool> {
    let mut wrap_allowed_after: Vec<bool> = vec![false; chars.len()];

    // Always treat space/tab as wrap opportunities.
    for (i, &ch) in chars.iter().enumerate() {
        if ch == ' ' || ch == '\t' {
            wrap_allowed_after[i] = true;
        }
    }

    for tok in tokens {
        if tok.is_empty() {
            continue;
        }

        // Skip whitespace here; already handled above.
        if tok == " " || tok == "\t" {
            continue;
        }

        let tok_chars: Vec<char> = tok.chars().collect();
        if tok_chars.is_empty() {
            continue;
        }

        if tok_chars.len() > chars.len() {
            continue;
        }

        for start in 0..=(chars.len() - tok_chars.len()) {
            let mut matched = true;
            for (j, &tch) in tok_chars.iter().enumerate() {
                if chars[start + j] != tch {
                    matched = false;
                    break;
                }
            }
            if matched {
                let end = start + tok_chars.len() - 1;
                wrap_allowed_after[end] = true;
            }
        }
    }

    wrap_allowed_after
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

fn uvs_for_glyph(ch: char) -> Vec<[f32; 2]> {
    const COLS: f32 = 16.0;
    const ROWS: f32 = 16.0;

    // Atlas layout is ASCII-order in a 16x16 grid:
    // row = ascii_code / 16, col = ascii_code % 16
    // e.g. '!': 33 => row 2, col 1 (with the two initial blank/control rows).
    let code: u8 = if ch.is_ascii() {
        ch as u8
    } else {
        b'?' // fallback
    };
    let row = (code / 16) as f32;
    let col = (code % 16) as f32;

    let u0 = col / COLS;
    let u1 = (col + 1.0) / COLS;

    // Atlas convention for `assets/textures/font.dds`:
    // - Row 0 is the TOP row of the image.
    // - Our texture sampling treats v=0 as TOP and v=1 as BOTTOM.
    let v0 = row / ROWS;
    let v1 = (row + 1.0) / ROWS;

    // Quad vertex order from MeshFactory::quad_2d():
    // 0 bottom-left, 1 bottom-right, 2 top-right, 3 top-left
    // Since row 0 is the *top* of the atlas, bottom vertices must use v1.
    vec![[u0, v1], [u1, v1], [u1, v0], [u0, v0]]
}
