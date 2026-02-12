use crate::engine::ecs::component::{
    AudioGainComponent, AudioHighPassFilterComponent, AudioLimiterComponent,
    AudioLowPassFilterComponent, AudioMixComponent, AudioOscillatorComponent,
};
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone)]
pub struct CompiledAudioGraph {
    pub root: AudioGraphNode,
}

#[derive(Debug, Clone)]
pub struct AudioGraphNode {
    pub component: ComponentId,
    pub kind: AudioGraphNodeKind,
    pub mix: Option<AudioMixSpec>,
    pub children: Vec<AudioGraphNode>,
}

#[derive(Debug, Clone)]
pub struct AudioMixSpec {
    pub component: ComponentId,
    pub weights: Vec<f32>,
}

#[derive(Debug, Clone)]
pub enum AudioGraphNodeKind {
    OscillatorSource {
        voices: usize,
    },
    Gain {
        gain: f32,
    },
    LowPass {
        cutoff_hz: f32,
        resonance: f32,
    },
    HighPass {
        cutoff_hz: f32,
        resonance: f32,
    },
    Limiter {
        attack_ms: f32,
        release_ms: f32,
        threshold: f32,
    },
}

#[derive(Debug)]
pub enum CompileAudioGraphError {
    NotAnAudioSource(ComponentId),
}

impl std::fmt::Display for CompileAudioGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileAudioGraphError::NotAnAudioSource(cid) => {
                write!(f, "component {cid:?} is not an AudioSource")
            }
        }
    }
}

impl std::error::Error for CompileAudioGraphError {}

pub struct AudioGraphCompiler;

impl AudioGraphCompiler {
    pub fn compile(
        world: &World,
        source_root: ComponentId,
    ) -> Result<CompiledAudioGraph, CompileAudioGraphError> {
        if let Some(src) = world.get_component_by_id_as::<AudioOscillatorComponent>(source_root) {
            let kind = AudioGraphNodeKind::OscillatorSource {
                voices: src.oscillators.len(),
            };
            let (mix, children) = Self::compile_effect_children(world, source_root);
            return Ok(CompiledAudioGraph {
                root: AudioGraphNode {
                    component: source_root,
                    kind,
                    mix,
                    children,
                },
            });
        }

        Err(CompileAudioGraphError::NotAnAudioSource(source_root))
    }

    fn compile_effect_children(
        world: &World,
        parent: ComponentId,
    ) -> (Option<AudioMixSpec>, Vec<AudioGraphNode>) {
        let mut child_ids: Vec<ComponentId> = world.children_of(parent).iter().copied().collect();
        child_ids.sort();

        let mut mix: Option<AudioMixSpec> = None;
        for &cid in &child_ids {
            if let Some(m) = world.get_component_by_id_as::<AudioMixComponent>(cid) {
                // First one wins (deterministic due to sorting).
                mix = Some(AudioMixSpec {
                    component: cid,
                    weights: m.weights.clone(),
                });
                break;
            }
        }

        let effect_children: Vec<AudioGraphNode> = child_ids
            .into_iter()
            .filter(|&cid| {
                // Exclude mix metadata nodes from the effect branch list.
                world
                    .get_component_by_id_as::<AudioMixComponent>(cid)
                    .is_none()
            })
            .filter(|&cid| Self::effect_kind(world, cid).is_some())
            .filter_map(|cid| Self::compile_effect_node(world, cid))
            .collect();

        (mix, effect_children)
    }

    fn compile_effect_node(world: &World, effect_cid: ComponentId) -> Option<AudioGraphNode> {
        let kind = Self::effect_kind(world, effect_cid)?;
        let (mix, children) = Self::compile_effect_children(world, effect_cid);
        Some(AudioGraphNode {
            component: effect_cid,
            kind,
            mix,
            children,
        })
    }

    fn effect_kind(world: &World, cid: ComponentId) -> Option<AudioGraphNodeKind> {
        if let Some(c) = world.get_component_by_id_as::<AudioGainComponent>(cid) {
            return Some(AudioGraphNodeKind::Gain { gain: c.gain });
        }
        if let Some(c) = world.get_component_by_id_as::<AudioLowPassFilterComponent>(cid) {
            return Some(AudioGraphNodeKind::LowPass {
                cutoff_hz: c.cutoff_hz,
                resonance: c.resonance,
            });
        }
        if let Some(c) = world.get_component_by_id_as::<AudioHighPassFilterComponent>(cid) {
            return Some(AudioGraphNodeKind::HighPass {
                cutoff_hz: c.cutoff_hz,
                resonance: c.resonance,
            });
        }
        if let Some(c) = world.get_component_by_id_as::<AudioLimiterComponent>(cid) {
            return Some(AudioGraphNodeKind::Limiter {
                attack_ms: c.attack_ms,
                release_ms: c.release_ms,
                threshold: c.threshold,
            });
        }
        None
    }
}

impl CompiledAudioGraph {
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.root.pretty_into(&mut out, 0);
        out
    }
}

impl AudioGraphNode {
    fn pretty_into(&self, out: &mut String, indent: usize) {
        let pad = "  ".repeat(indent);
        let line = match &self.kind {
            AudioGraphNodeKind::OscillatorSource { voices } => {
                format!(
                    "{pad}- AudioOscillatorComponent {{ oscillators: <len={voices}> }} (component={:?})\n",
                    self.component
                )
            }
            AudioGraphNodeKind::Gain { gain } => {
                format!(
                    "{pad}- AudioGainComponent {{ gain: {gain:.3} }} (component={:?})\n",
                    self.component
                )
            }
            AudioGraphNodeKind::LowPass {
                cutoff_hz,
                resonance,
            } => {
                format!(
                    "{pad}- AudioLowPassFilterComponent {{ cutoff_hz: {cutoff_hz:.1}, resonance: {resonance:.3} }} (component={:?})\n",
                    self.component
                )
            }
            AudioGraphNodeKind::HighPass {
                cutoff_hz,
                resonance,
            } => {
                format!(
                    "{pad}- AudioHighPassFilterComponent {{ cutoff_hz: {cutoff_hz:.1}, resonance: {resonance:.3} }} (component={:?})\n",
                    self.component
                )
            }
            AudioGraphNodeKind::Limiter {
                attack_ms,
                release_ms,
                threshold,
            } => format!(
                "{pad}- AudioLimiterComponent {{ attack_ms: {attack_ms:.1}, release_ms: {release_ms:.1}, threshold: {threshold:.3} }} (component={:?})\n",
                self.component
            ),
        };
        out.push_str(&line);

        if self.children.len() > 1 {
            let mut weights: Vec<f32> = Vec::with_capacity(self.children.len());
            for i in 0..self.children.len() {
                let w = self
                    .mix
                    .as_ref()
                    .map(|m| m.weights.get(i).copied().unwrap_or(1.0))
                    .unwrap_or(1.0);
                weights.push(w);
            }

            if let Some(m) = &self.mix {
                out.push_str(&format!(
                    "{pad}  mix: AudioMixComponent {{ weights: {:?} }} (component={:?})\n",
                    weights, m.component
                ));
            } else {
                out.push_str(&format!(
                    "{pad}  mix: <implicit> (weights: {:?})\n",
                    weights
                ));
            }
        }

        for (i, ch) in self.children.iter().enumerate() {
            if self.children.len() > 1 {
                out.push_str(&format!("{pad}  [branch {i}]\n"));
            }
            ch.pretty_into(out, indent + 2);
        }
    }
}
