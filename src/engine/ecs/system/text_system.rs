use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, OpacityComponent, RaycastableComponent, RenderableComponent,
    TextComponent, TextShadowComponent, TextureComponent,
    TextureFilteringComponent, TransformComponent, TransparentCutoutComponent, UVComponent,
};
use crate::engine::ecs::{EventSignal, IntentValue};
use crate::engine::graphics::TextureFiltering;
use crate::engine::graphics::VisualWorld;

#[derive(Debug, Default)]
pub struct TextSystem;

#[derive(Debug, Clone, Copy)]
struct WordWrapState {
    col: usize,
    row: usize,
    /// Maximum column reached on any line (used for background width).
    max_col: usize,
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
            max_col: 0,
            line_count: 0,
            last_wrap_allowed: false,
            wrap_at,
            word_wrap,
        }
    }

    fn newline(&mut self) {
        self.max_col = self.max_col.max(self.col);
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
            self.max_col = self.max_col.max(self.col);
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

    /// Word-wrap look-ahead. Called before each non-space glyph at index `i`
    /// with the length of the unbreakable run starting at `i` (number of
    /// chars until the next wrap-allowed position or end-of-text).
    ///
    /// If the upcoming word wouldn't fit on the current line and we just
    /// passed a wrap opportunity, wrap *now* — `apply_wrap_if_needed` only
    /// catches the overflow after it has already happened, which leaves the
    /// trailing word sticking past the container.
    fn apply_word_wrap_lookahead(&mut self, next_word_len: usize) {
        if !self.word_wrap || self.wrap_at == 0 || self.col == 0 {
            return;
        }
        if !self.last_wrap_allowed {
            return;
        }
        if self.col + next_word_len > self.wrap_at {
            self.max_col = self.max_col.max(self.col);
            self.row += 1;
            self.col = 0;
            self.line_count = 0;
            self.last_wrap_allowed = false;
        }
    }
}

/// For each index `i`, the number of chars from `i` until (and not
/// including) the next wrap-allowed position, or `chars.len() - i` if no
/// further break exists. This is the "word length" the look-ahead checks
/// against `wrap_at` to decide whether to break at the preceding space.
fn compute_word_run_len(wrap_allowed_after: &[bool]) -> Vec<usize> {
    let n = wrap_allowed_after.len();
    let mut out = vec![0; n];
    let mut run = 0usize;
    for i in (0..n).rev() {
        if wrap_allowed_after[i] {
            run = 0;
        } else {
            run += 1;
        }
        out[i] = run;
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub struct SpawnedGlyph {
    pub transform: ComponentId,
    pub renderable: ComponentId,
    pub uv: ComponentId,
    pub texture: ComponentId,
}

impl TextSystem {
    pub(crate) fn on_parent_changed(
        world: &mut World,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        env: &crate::engine::ecs::Signal,
    ) {
        let Some(EventSignal::ParentChanged {
            child, new_parent, ..
        }) = env.event.as_ref()
        else {
            return;
        };

        // Only care about style nodes being attached directly under a TextComponent root.
        let Some(parent) = *new_parent else {
            return;
        };

        if world
            .get_component_by_id_as::<TextComponent>(parent)
            .is_none()
        {
            return;
        }

        // Late-attached ColorComponent: trigger re-registration so existing glyph renderables
        // update immediately.
        if world
            .get_component_by_id_as::<ColorComponent>(*child)
            .is_some()
        {
            emit.push_intent_now(
                *child,
                IntentValue::RegisterColor {
                    component_ids: vec![*child],
                },
            );
        }
    }

    fn spawn_glyph_quad(
        world: &mut World,
        parent: ComponentId,
        uvs: Vec<[f32; 2]>,
        texture_uri: &str,
        filtering: Option<TextureFiltering>,
        emissive: Option<bool>,
        raycastable: Option<RaycastableComponent>,
        color_override: Option<[f32; 4]>,
    ) -> (ComponentId, ComponentId, ComponentId) {
        // Optional color override: insert a ColorComponent above the renderable.
        let renderable_parent = if let Some(rgba) = color_override {
            let c_id = world.add_component(ColorComponent { rgba });
            let _ = world.add_child(parent, c_id);
            c_id
        } else {
            parent
        };

        let r_id = world.add_component(RenderableComponent::square());
        let _ = world.add_child(renderable_parent, r_id);

        if let Some(rc) = raycastable {
            let rc_id = world.add_component(rc);
            let _ = world.add_child(r_id, rc_id);
        }

        let uv_id = world.add_component(UVComponent { uvs });
        let _ = world.add_child(r_id, uv_id);

        let tex_id = world.add_component(TextureComponent::with_uri(texture_uri.to_string()));
        let _ = world.add_child(r_id, tex_id);

        if let Some(filtering) = filtering {
            let f_id = world.add_component(TextureFilteringComponent::new(filtering));
            let _ = world.add_child(r_id, f_id);
        }

        if let Some(enabled) = emissive {
            let e_id = world.add_component(EmissiveComponent::new(if enabled { 1.0 } else { 0.0 }));
            let _ = world.add_child(r_id, e_id);
        }

        (r_id, uv_id, tex_id)
    }

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
                .map(|e| e.intensity > 0.0)
        });

        // Raycasting is explicit opt-in. For text, allow toggling at the TextComponent root by
        // attaching an immediate RaycastableComponent child; this is propagated to all glyphs.
        // The full component is copied so that PointerEvents (click_only, drag_only, etc.) is preserved.
        let inherited_raycastable = world.children_of(component).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<RaycastableComponent>(ch)
                .copied()
                .filter(|r| r.enable)
        });

        // Optional per-glyph shadow pass.
        // Requested topology: TextShadowComponent is parented to the TextComponent.
        let shadow: Option<TextShadowComponent> =
            world.children_of(component).iter().find_map(|&ch| {
                world
                    .get_component_by_id_as::<TextShadowComponent>(ch)
                    .copied()
            });

        // Mark built immediately to avoid re-entrancy/double-build.
        if let Some(text_comp) = world.get_component_by_id_as_mut::<TextComponent>(component) {
            text_comp.mark_built();
        }

        let mut spawned = Vec::new();

        let chars: Vec<char> = text.chars().collect();
        let wrap_allowed_after: Vec<bool> = compute_wrap_allowed_after(&chars, &word_wrap_tokens);
        let word_run_len = compute_word_run_len(&wrap_allowed_after);

        let mut wrap_state = WordWrapState::new(wrap_at, word_wrap);

        for (i, ch) in chars.iter().copied().enumerate() {
            if !Self::handle_word_wrap_for(ch, i, &wrap_allowed_after, &mut wrap_state) {
                continue;
            }
            wrap_state.apply_word_wrap_lookahead(word_run_len.get(i).copied().unwrap_or(1));

            let (x, y) = wrap_state.cursor_pos();
            let t = TransformComponent::new().with_position(x, y, 0.0);
            let t_id = world.add_component(t);
            let _ = world.add_child(component, t_id);

            let glyph_uvs = uvs_for_glyph(ch);
            let (r_id, uv_id, tex_id) = Self::spawn_glyph_quad(
                world,
                t_id,
                glyph_uvs.clone(),
                &inherited_font_texture_uri,
                inherited_filtering,
                inherited_emissive,
                inherited_raycastable,
                None,
            );

            if let Some(shadow) = shadow {
                let z_back = -shadow.offset[2].abs();
                let mut spawn_shadow = |scale: f32, z: f32| {
                    let ot = TransformComponent::new()
                        .with_position(shadow.offset[0], shadow.offset[1], z)
                        .with_scale(scale, scale, 1.0);
                    let ot_id = world.add_component(ot);
                    let _ = world.add_child(t_id, ot_id);

                    // Shadow quad: no raycasting by default.
                    let _ = Self::spawn_glyph_quad(
                        world,
                        ot_id,
                        glyph_uvs.clone(),
                        &inherited_font_texture_uri,
                        inherited_filtering,
                        inherited_emissive,
                        None,
                        Some(shadow.rgba),
                    );
                };

                // If the shadow is expanded (>1.0), spawn two shadow glyphs.
                if shadow.scale > 1.0 {
                    spawn_shadow(1.0 / (shadow.scale * 1.3), z_back);
                    spawn_shadow(shadow.scale, z_back * 2.0);
                } else {
                    spawn_shadow(shadow.scale, z_back);
                }
            }

            spawned.push(SpawnedGlyph {
                transform: t_id,
                renderable: r_id,
                uv: uv_id,
                texture: tex_id,
            });

            wrap_state.advance_glyph(i, &wrap_allowed_after);
        }

        // Finalize max_col to include the last (non-wrapped) line.
        wrap_state.max_col = wrap_state.max_col.max(wrap_state.col);

        spawned
    }

    /// Pure text measurement — runs wrap logic without spawning any glyphs.
    ///
    /// Returns `(max_col, line_count)`:
    /// - `max_col`    — widest line in character columns (for background sizing)
    /// - `line_count` — total number of lines after wrapping
    ///
    /// `wrap_at = 0` disables wrapping.
    pub fn measure(
        text: &str,
        wrap_at: usize,
        word_wrap: bool,
        word_wrap_tokens: &[String],
    ) -> (usize, usize) {
        let chars: Vec<char> = text.chars().collect();
        let wrap_allowed_after = compute_wrap_allowed_after(&chars, word_wrap_tokens);
        let word_run_len = compute_word_run_len(&wrap_allowed_after);
        let mut state = WordWrapState::new(wrap_at, word_wrap);

        for (i, ch) in chars.iter().copied().enumerate() {
            if ch == '\n' {
                state.newline();
                continue;
            }
            state.apply_wrap_if_needed();
            if ch == ' ' {
                state.advance_space(i, &wrap_allowed_after);
                continue;
            }
            if ch == '\t' {
                state.advance_tab(i, &wrap_allowed_after);
                continue;
            }
            state.apply_word_wrap_lookahead(word_run_len.get(i).copied().unwrap_or(1));
            state.advance_glyph(i, &wrap_allowed_after);
        }

        state.max_col = state.max_col.max(state.col);
        (state.max_col, state.row + 1)
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

    // Atlas convention for `assets/textures/font_system.dds` (and `font.dds`):
    // - Row 0 is the TOP row of the image.
    // - Our texture sampling treats v=0 as TOP and v=1 as BOTTOM.
    // - Each glyph is centered within its 1/16 × 1/16 cell (no baseline offset);
    //   layout therefore treats the quad as 1×1 with the letter centered at the quad's center.
    let v0 = row / ROWS;
    let v1 = (row + 1.0) / ROWS;

    // Quad vertex order from MeshFactory::quad_2d():
    // 0 bottom-left, 1 bottom-right, 2 top-right, 3 top-left
    // Since row 0 is the *top* of the atlas, bottom vertices must use v1.
    vec![[u0, v1], [u1, v1], [u1, v0], [u0, v0]]
}
