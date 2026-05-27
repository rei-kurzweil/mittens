use super::{Component, ComponentRef};
use crate::engine::ecs::{ComponentId, World};

/// Scope wrapper declaring the audio sources (voices) available to
/// descendant `MusicNoteComponent`s.
///
/// See docs/spec/audio-sources.md §6.6. Voices are ordered so the first
/// declared voice is the implicit default (rank 4 in the resolution
/// precedence table). Named lookup is preferred over positional — adding
/// or removing voices won't silently shuffle assignments.
#[derive(Debug, Clone, Default)]
pub struct MusicContextComponent {
    /// Durable authored voices, in declaration order.
    pub voices: Vec<(String, ComponentRef)>,

    /// Cache of resolved voice targets. Same length and order as `voices`
    /// when populated; `None` slots mean "ref present, not yet resolved".
    pub voices_resolved: Vec<Option<ComponentId>>,

    component: Option<ComponentId>,
}

impl MusicContextComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_voice(mut self, name: impl Into<String>, source: ComponentRef) -> Self {
        self.voices.push((name.into(), source));
        self.voices_resolved.push(None);
        self
    }

    pub fn add_voice(&mut self, name: impl Into<String>, source: ComponentRef) {
        self.voices.push((name.into(), source));
        self.voices_resolved.push(None);
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }

    /// Look up a voice by name or, if `name` is None, by position 0.
    /// Resolves the underlying `ComponentRef` on demand and caches the
    /// result. Returns `None` if the voice doesn't exist or its ref can't
    /// be resolved yet.
    pub fn lookup_voice(&mut self, world: &World, name: Option<&str>) -> Option<ComponentId> {
        let idx = match name {
            None => 0,
            Some(n) => self.voices.iter().position(|(vn, _)| vn == n)?,
        };
        if let Some(cached) = self.voices_resolved.get(idx).copied().flatten() {
            return Some(cached);
        }
        let (_, src) = self.voices.get(idx)?;
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
        if let Some(slot) = self.voices_resolved.get_mut(idx) {
            *slot = resolved;
        }
        resolved
    }
}

impl Component for MusicContextComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "music_context"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self, _world: &World) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        use crate::meow_meow::ast::Expression;

        fn ref_expr(t: &ComponentRef) -> Expression {
            match t {
                ComponentRef::Guid(u) => Expression::String(format!("@uuid:{u}")),
                ComponentRef::Query(sel) => Expression::String(sel.clone()),
            }
        }

        let mut c = ce(self.name());
        for (name, src) in &self.voices {
            c = c.with_call("voice", vec![s(name), ref_expr(src)]);
        }
        c
    }
}
