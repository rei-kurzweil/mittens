use super::{Component, ComponentRef};
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NotePitch {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MusicNote {
    duration: f32,
    #[serde(default = "MusicNote::default_velocity")]
    velocity: f32,
    pitch: NotePitch,
    octave: u16,
}

impl Default for MusicNote {
    fn default() -> Self {
        Self {
            duration: 0.25,
            velocity: 1.0,
            pitch: NotePitch::A,
            octave: 4,
        }
    }
}

impl MusicNote {
    fn default_velocity() -> f32 {
        1.0
    }

    pub fn duration_beats(&self) -> f32 {
        self.duration
    }

    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    pub fn octave(&self) -> u16 {
        self.octave
    }

    pub fn pitch_name(&self) -> &'static str {
        match self.pitch {
            NotePitch::A => "a",
            NotePitch::B => "b",
            NotePitch::C => "c",
            NotePitch::D => "d",
            NotePitch::E => "e",
            NotePitch::F => "f",
            NotePitch::G => "g",
        }
    }

    pub fn with_duration_beats(mut self, duration_beats: f32) -> Self {
        self.duration = duration_beats;
        self
    }

    pub fn with_velocity(mut self, velocity: f32) -> Self {
        self.velocity = if velocity.is_finite() {
            velocity.max(0.0)
        } else {
            1.0
        };
        self
    }

    pub fn with_octave(mut self, octave: u16) -> Self {
        self.octave = octave;
        self
    }

    pub fn a(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::A,
            octave,
        }
    }

    pub fn b(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::B,
            octave,
        }
    }

    pub fn c(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::C,
            octave,
        }
    }

    pub fn d(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::D,
            octave,
        }
    }

    pub fn e(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::E,
            octave,
        }
    }

    pub fn f(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::F,
            octave,
        }
    }

    pub fn g(octave: u16, duration_beats: f32) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch: NotePitch::G,
            octave,
        }
    }

    pub(crate) fn from_pitch(duration_beats: f32, pitch: NotePitch, octave: u16) -> Self {
        Self {
            duration: duration_beats,
            velocity: 1.0,
            pitch,
            octave,
        }
    }

    pub(crate) fn pitch(&self) -> NotePitch {
        self.pitch
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MusicNoteComponent {
    pub note: MusicNote,

    /// Pre-authored target audio source (`audio_source` per spec §6.4.1).
    /// Durable ref preserved through serde / MMS dump. `target_resolved` is
    /// the runtime cache filled by `resolve_target`. When both are None,
    /// dispatch falls back to ancestor-walk per docs/spec/audio-sources.md
    /// §6.6 rank 5 (the executor caches the result back here).
    #[serde(skip)]
    pub target_source: Option<ComponentRef>,

    /// Runtime cache of the resolved target. Skipped by serde — fresh
    /// loads re-resolve from `target_source` (or ancestor walk) on first
    /// use, then this stays populated.
    #[serde(skip)]
    pub target_resolved: Option<ComponentId>,

    /// Pre-authored default beat (`scheduled_beat` per spec §6.4.1). When
    /// None, `.play()` fires immediately (beat_offset = 0).
    #[serde(default)]
    pub scheduled_beat: Option<f64>,

    /// Fire `AudioSchedulePlay` automatically when this component initializes.
    /// Defaults to false — silent unless explicitly triggered.
    #[serde(default)]
    pub play_on_attach: bool,

    /// Voice name for `MusicContext` lookup (rank 3 / rank 4 in the
    /// resolution precedence). Used only when `target_source` is None;
    /// matches against the nearest ancestor `MusicContextComponent`'s
    /// `voices` map. `None` means "use the context's first voice".
    #[serde(default)]
    pub voice_name: Option<String>,

    #[serde(skip)]
    component: Option<ComponentId>,
}

impl MusicNoteComponent {
    pub fn new(note: MusicNote) -> Self {
        Self {
            note,
            target_source: None,
            target_resolved: None,
            scheduled_beat: None,
            play_on_attach: false,
            voice_name: None,
            component: None,
        }
    }

    pub fn with_voice_name(mut self, name: impl Into<String>) -> Self {
        self.voice_name = Some(name.into());
        self
    }

    pub fn from_note(note: MusicNote) -> Self {
        Self::new(note)
    }

    pub fn with_target_source(mut self, source: ComponentRef) -> Self {
        self.target_source = Some(source);
        self
    }

    pub fn with_scheduled_beat(mut self, beat: f64) -> Self {
        self.scheduled_beat = Some(beat);
        self
    }

    pub fn with_play_on_attach(mut self, on: bool) -> Self {
        self.play_on_attach = on;
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }

    /// Resolve a target for this note following docs/spec/audio-sources.md
    /// §6.6 precedence: cached → `target_source` (Guid/Query) → nearest
    /// ancestor `MusicContextComponent` (named voice if `voice_name` set,
    /// else voice 0). Returns `None` when nothing resolves — the
    /// executor's ancestor-walk fallback (rank 5) handles that case and
    /// writes the result back to `target_resolved` afterward.
    pub fn resolve_target(&mut self, world: &mut World) -> Option<ComponentId> {
        if self.target_resolved.is_some() {
            return self.target_resolved;
        }
        if let Some(src) = self.target_source.as_ref() {
            let resolved = match src {
                ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
                ComponentRef::Query(selector) => {
                    let roots: Vec<ComponentId> = world
                        .all_components()
                        .filter(|&cid| world.parent_of(cid).is_none())
                        .collect();
                    roots
                        .into_iter()
                        .find_map(|root| world.find_component(root, selector))
                }
            };
            self.target_resolved = resolved;
            if resolved.is_some() {
                return resolved;
            }
        }
        // Rank 4: walk ancestors for the nearest MusicContextComponent and
        // look up our voice (named, or position 0 when unnamed).
        let self_id = self.component?;
        let ctx_id = find_music_context_ancestor(world, self_id)?;
        let voice = self.voice_name.clone();
        let resolved = {
            let ctx = world.get_component_by_id_as_mut::<super::MusicContextComponent>(ctx_id)?;
            // Defer the lookup: cache check is cheap, but ref resolution
            // needs `&World`, which we already borrowed mutably above.
            let cached = match &voice {
                None => ctx.voices_resolved.first().copied().flatten(),
                Some(n) => ctx
                    .voices
                    .iter()
                    .position(|(vn, _)| vn == n)
                    .and_then(|i| ctx.voices_resolved.get(i).copied().flatten()),
            };
            if cached.is_some() { cached } else { None }
        };
        if let Some(r) = resolved {
            self.target_resolved = Some(r);
            return Some(r);
        }
        // Cold path: ask the context to resolve + cache. Re-borrow.
        let resolved = {
            let ctx = world.get_component_by_id_as_mut::<super::MusicContextComponent>(ctx_id)?;
            // SAFETY of borrow: lookup_voice needs &World too. We grab voice
            // entries by clone, drop the mut borrow, then read-only world.
            let entry = match &voice {
                None => ctx.voices.first().cloned(),
                Some(n) => ctx.voices.iter().find(|(vn, _)| vn == n).cloned(),
            };
            entry
        };
        let (_, src) = resolved?;
        let id = match src {
            ComponentRef::Guid(uuid) => world.component_id_by_guid(uuid),
            ComponentRef::Query(selector) => {
                let roots: Vec<ComponentId> = world
                    .all_components()
                    .filter(|&cid| world.parent_of(cid).is_none())
                    .collect();
                roots
                    .into_iter()
                    .find_map(|root| world.find_component(root, &selector))
            }
        };
        // Cache into the context too, so other notes benefit.
        if let (Some(id), Some(ctx)) = (
            id,
            world.get_component_by_id_as_mut::<super::MusicContextComponent>(ctx_id),
        ) {
            let idx = match &voice {
                None => Some(0),
                Some(n) => ctx.voices.iter().position(|(vn, _)| vn == n),
            };
            if let Some(idx) = idx {
                if let Some(slot) = ctx.voices_resolved.get_mut(idx) {
                    *slot = Some(id);
                }
            }
        }
        self.target_resolved = id;
        id
    }
}

fn find_music_context_ancestor(world: &World, id: ComponentId) -> Option<ComponentId> {
    let mut cur = id;
    while let Some(p) = world.parent_of(cur) {
        if world
            .get_component_by_id_as::<super::MusicContextComponent>(p)
            .is_some()
        {
            return Some(p);
        }
        cur = p;
    }
    None
}

impl Default for MusicNoteComponent {
    fn default() -> Self {
        Self::new(MusicNote::default())
    }
}

impl Component for MusicNoteComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "music_note"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        if !self.play_on_attach {
            return;
        }
        // Per docs/spec/audio-sources.md §6.4: play_on_attach fires
        // AudioSchedulePlay when the component initializes. Target resolution
        // is deferred to the executor — `target_resolved` is filled either
        // by `resolve_target` (when `target_source` is set) or by the
        // executor's ancestor-walk writeback (when neither is set).
        // Passing `component` (self id) here lets the executor walk both
        // subtree and ancestor scopes.
        let target = self.target_resolved.unwrap_or(component);
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::AudioSchedulePlay {
                component_ids: vec![target],
                beat_offset: 0.0,
                beat_context: self.scheduled_beat,
                note: Some(self.note),
                gain: None,
                rate: None,
                duration: None,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let pitch = self.note.pitch_name();
        let mut c = ce_call(
            "MusicNote",
            pitch,
            vec![
                num(self.note.octave() as f64),
                num(self.note.duration_beats() as f64),
            ],
        );
        if (self.note.velocity() - 1.0).abs() > f32::EPSILON {
            c = c.with_call("velocity", vec![num(self.note.velocity() as f64)]);
        }
        if self.play_on_attach {
            c = c.with_call("play_on_attach", vec![]);
        }
        if let Some(b) = self.scheduled_beat {
            c = c.with_call("at_beat", vec![num(b)]);
        }
        c
    }
}
