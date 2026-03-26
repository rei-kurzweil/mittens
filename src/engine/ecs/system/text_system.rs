use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, OpacityComponent, RaycastableComponent, RenderableComponent,
    TextBackgroundComponent, TextComponent, TextShadowComponent, TextureComponent,
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
        raycastable: Option<bool>,
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

        if let Some(enable) = raycastable {
            let rc_id = world.add_component(RaycastableComponent::new(enable));
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
            let e_id = world.add_component(EmissiveComponent { enabled });
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
                .map(|e| e.enabled)
        });

        // Raycasting is explicit opt-in. For text, allow toggling at the TextComponent root by
        // attaching an immediate RaycastableComponent child; this is propagated to all glyphs.
        let inherited_raycastable = world.children_of(component).iter().find_map(|&ch| {
            world
                .get_component_by_id_as::<RaycastableComponent>(ch)
                .map(|r| r.enable)
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

        let mut wrap_state = WordWrapState::new(wrap_at, word_wrap);

        for (i, ch) in chars.iter().copied().enumerate() {
            if !Self::handle_word_wrap_for(ch, i, &wrap_allowed_after, &mut wrap_state) {
                continue;
            }

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

        // Spawn a background quad if a TextBackgroundComponent is present.
        let background: Option<(ComponentId, TextBackgroundComponent)> =
            world.children_of(component).iter().find_map(|&ch| {
                world
                    .get_component_by_id_as::<TextBackgroundComponent>(ch)
                    .copied()
                    .map(|bg| (ch, bg))
            });

        if let Some((bg_id, bg)) = background {
            let cols = wrap_state.max_col as f32;
            let rows = (wrap_state.row + 1) as f32;

            if cols > 0.0 {
                // Color comes from an optional ColorComponent child of the TextBackgroundComponent.
                // Alpha drives the OpacityComponent that routes the quad into the transparent pass.
                const DEFAULT_BG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.75];
                let color = world
                    .children_of(bg_id)
                    .iter()
                    .find_map(|&ch| {
                        world
                            .get_component_by_id_as::<ColorComponent>(ch)
                            .map(|c| c.rgba)
                    })
                    .unwrap_or(DEFAULT_BG_COLOR);

                // Glyph grid X spans [-0.5, cols-0.5], Y spans [0.5, -(rows-0.5)].
                // Background edges: left = -0.5 - pad_left, right = cols-0.5 + pad_right,
                //                   top  =  0.5 + pad_top,  bottom = -(rows-0.5) - pad_bottom.
                let w = cols + bg.padding_left + bg.padding_right;
                let h = rows + bg.padding_top + bg.padding_bottom;
                let cx = (cols - 1.0 + bg.padding_right - bg.padding_left) / 2.0;
                let cy = -(rows - 1.0 + bg.padding_bottom - bg.padding_top) / 2.0;

                let bg_t = world.add_component(
                    TransformComponent::new()
                        .with_position(cx, cy, bg.z_offset)
                        .with_scale(w, h, 1.0),
                );
                let _ = world.add_child(component, bg_t);

                let bg_col = world.add_component(ColorComponent {
                    rgba: [color[0], color[1], color[2], 1.0],
                });
                let _ = world.add_child(bg_t, bg_col);

                let bg_r = world.add_component(RenderableComponent::square());
                let _ = world.add_child(bg_col, bg_r);

                // Route the quad into the transparent pass.
                let bg_op = world
                    .add_component(OpacityComponent::new().with_opacity(color[3]));
                let _ = world.add_child(bg_r, bg_op);
                // Explicitly opt out of cutout so the background is never routed
                // to the alpha-to-coverage pass even when a parent TextComponent
                // has TransparentCutoutComponent enabled.
                let bg_tc = world.add_component(TransparentCutoutComponent { enabled: false });
                let _ = world.add_child(bg_r, bg_tc);
            }
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
