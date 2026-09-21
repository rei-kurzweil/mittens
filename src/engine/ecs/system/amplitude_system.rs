use std::collections::{HashMap, VecDeque};

use crate::engine::ecs::component::volume_normalization::LiveVolumeNormalizationPolicy;
use crate::engine::ecs::component::{
    AmplitudeComponent, AmplitudeSample, AmplitudeStatus, AudioClipComponent, AudioInputComponent,
    AudioOscillatorComponent, QueryRootMode, VolumeNormalizationComponent,
    VolumeNormalizationReason, resolve_component_ref,
};
use crate::engine::ecs::{ComponentId, World};
use std::sync::Arc;

/// A source-runtime measurement awaiting main-thread validation and retention.
/// The future real-time handoff feeds this same bounded protocol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmplitudeSnapshot {
    pub observer: ComponentId,
    pub source: ComponentId,
    pub sample: AmplitudeSample,
}

/// A normalized callback result awaiting the same generation validation as a
/// raw amplitude observation.  The callback never touches the component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VolumeNormalizationSnapshot {
    pub normalizer: ComponentId,
    pub source: ComponentId,
    pub sample: AmplitudeSample,
    pub gain_db: f32,
    pub reason: VolumeNormalizationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConsumerState {
    source: ComponentId,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputAmplitudeConsumer {
    pub observer: ComponentId,
    pub source: ComponentId,
    pub generation: u64,
    pub window_sec: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct InputNormalizationConsumer {
    pub normalizer: ComponentId,
    pub source: ComponentId,
    pub generation: u64,
    pub window_sec: f32,
    pub policy: Arc<LiveVolumeNormalizationPolicy>,
}

impl PartialEq for InputNormalizationConsumer {
    fn eq(&self, other: &Self) -> bool {
        self.normalizer == other.normalizer
            && self.source == other.source
            && self.generation == other.generation
            && self.window_sec == other.window_sec
            && Arc::ptr_eq(&self.policy, &other.policy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NormalizerState {
    source: ComponentId,
    generation: u64,
    upstream: ComponentId,
    upstream_generation: u64,
}

/// Main-thread ownership boundary for amplitude observation state.
///
/// It deliberately owns no audio stream, callback buffer, or accumulator. The
/// capture slice will replace `submit_snapshot`'s test/control path with a
/// bounded RT queue while preserving the validation below.
#[derive(Debug)]
pub struct AmplitudeSystem {
    consumers: HashMap<ComponentId, ConsumerState>,
    normalizers: HashMap<ComponentId, NormalizerState>,
    pending: VecDeque<AmplitudeSnapshot>,
    pending_normalized: VecDeque<VolumeNormalizationSnapshot>,
    dropped_snapshots: u64,
}

impl Default for AmplitudeSystem {
    fn default() -> Self {
        Self {
            consumers: HashMap::new(),
            normalizers: HashMap::new(),
            pending: VecDeque::new(),
            pending_normalized: VecDeque::new(),
            dropped_snapshots: 0,
        }
    }
}

impl AmplitudeSystem {
    pub const MAX_PENDING_SNAPSHOTS: usize = 256;

    pub fn new() -> Self {
        Self::default()
    }

    /// Bounded source-runtime handoff. This is public for deterministic source
    /// fixtures; production PCM integration will call it via its RT-safe queue
    /// drain, never directly from a callback.
    pub fn submit_snapshot(&mut self, snapshot: AmplitudeSnapshot) {
        if self.pending.len() == Self::MAX_PENDING_SNAPSHOTS {
            self.pending.pop_front();
            self.dropped_snapshots = self.dropped_snapshots.wrapping_add(1);
        }
        self.pending.push_back(snapshot);
    }

    pub(crate) fn submit_normalized_snapshot(&mut self, snapshot: VolumeNormalizationSnapshot) {
        if self.pending_normalized.len() == Self::MAX_PENDING_SNAPSHOTS {
            self.pending_normalized.pop_front();
            self.dropped_snapshots = self.dropped_snapshots.wrapping_add(1);
        }
        self.pending_normalized.push_back(snapshot);
    }

    pub fn dropped_snapshots(&self) -> u64 {
        self.dropped_snapshots
    }

    pub(crate) fn record_dropped_snapshots(&mut self, count: u64) {
        self.dropped_snapshots = self.dropped_snapshots.wrapping_add(count);
    }

    pub fn tick(&mut self, world: &mut World) {
        self.refresh_consumers(world);

        self.drain_pending(world);
    }

    pub(crate) fn drain_pending(&mut self, world: &mut World) {
        // Keep only the newest queued result per observer for this tick.
        let mut newest = HashMap::new();
        while let Some(snapshot) = self.pending.pop_front() {
            newest.insert(snapshot.observer, snapshot);
        }
        for (observer, snapshot) in newest {
            let Some(state) = self.consumers.get(&observer).copied() else {
                continue;
            };
            if state.source != snapshot.source || state.generation != snapshot.sample.generation {
                continue;
            }
            let Some(component) = world.get_component_by_id_as_mut::<AmplitudeComponent>(observer)
            else {
                continue;
            };
            let valid = match snapshot.sample.status {
                AmplitudeStatus::Live => snapshot.sample.is_live(),
                AmplitudeStatus::Neutral | AmplitudeStatus::Invalid => {
                    snapshot.sample.rms.is_finite() && snapshot.sample.peak.is_finite()
                }
                AmplitudeStatus::Pending => false,
            };
            if !valid || snapshot.sample.sequence < component.retained.sequence {
                continue;
            }
            component.retained = snapshot.sample;
        }

        // As with raw levels, retain only the newest result from each callback
        // unit.  Its source and generation must still match the live binding.
        let mut newest = HashMap::new();
        while let Some(snapshot) = self.pending_normalized.pop_front() {
            newest.insert(snapshot.normalizer, snapshot);
        }
        for (normalizer, snapshot) in newest {
            let Some(state) = self.normalizers.get(&normalizer).copied() else {
                continue;
            };
            if state.source != snapshot.source || state.generation != snapshot.sample.generation {
                continue;
            }
            let Some(component) =
                world.get_component_by_id_as_mut::<VolumeNormalizationComponent>(normalizer)
            else {
                continue;
            };
            let valid = match snapshot.sample.status {
                AmplitudeStatus::Live => snapshot.sample.is_live() && snapshot.gain_db.is_finite(),
                AmplitudeStatus::Neutral | AmplitudeStatus::Invalid => {
                    snapshot.sample.rms.is_finite() && snapshot.sample.peak.is_finite()
                }
                AmplitudeStatus::Pending => false,
            };
            if !valid || snapshot.sample.sequence < component.retained.sequence {
                continue;
            }
            component.retained = snapshot.sample;
            component.current_gain_db = snapshot.gain_db;
            component.adjustment_reason = snapshot.reason;
        }
    }

    pub(crate) fn refresh_consumers(&mut self, world: &mut World) {
        let ids: Vec<_> = world.all_components().collect();
        let mut live = HashMap::new();
        for id in ids {
            let Some(amplitude) = world.get_component_by_id_as::<AmplitudeComponent>(id) else {
                continue;
            };
            // A live ComponentId is stable across frames. Resolve its durable
            // selector only before first bind, or after deletion invalidated
            // the cached slot-map key.
            let source = amplitude
                .resolved_source
                .filter(|&source| is_audio_source(world, source))
                .or_else(|| {
                    amplitude.source.as_ref().and_then(|reference| {
                        resolve_component_ref(world, reference, Some(id), QueryRootMode::WorldRoot)
                    })
                });
            let source_is_valid =
                source.is_some_and(|source| is_audio_source_enabled(world, source));
            if !amplitude.enabled || !source_is_valid {
                if amplitude.retained.status != AmplitudeStatus::Invalid {
                    world
                        .get_component_by_id_as_mut::<AmplitudeComponent>(id)
                        .unwrap()
                        .bump_generation(AmplitudeStatus::Invalid);
                }
                continue;
            }
            let source = source.expect("validated above");
            let generation = amplitude.generation;
            if amplitude.resolved_source != Some(source) {
                world
                    .get_component_by_id_as_mut::<AmplitudeComponent>(id)
                    .unwrap()
                    .resolved_source = Some(source);
            }
            live.insert(id, ConsumerState { source, generation });
            if self.consumers.get(&id).copied() != Some(ConsumerState { source, generation }) {
                let component = world
                    .get_component_by_id_as_mut::<AmplitudeComponent>(id)
                    .unwrap();
                component.bump_generation(AmplitudeStatus::Pending);
                live.insert(
                    id,
                    ConsumerState {
                        source,
                        generation: component.generation,
                    },
                );
            }
        }
        self.consumers = live;

        // A normalizer is deliberately a separate input consumer: it owns a
        // distinct rolling window and controller even when its upstream raw
        // amplitude observes the same microphone.
        let ids: Vec<_> = world.all_components().collect();
        let mut live_normalizers = HashMap::new();
        for id in ids {
            let Some(normalization) =
                world.get_component_by_id_as::<VolumeNormalizationComponent>(id)
            else {
                continue;
            };
            let upstream = normalization.source.as_ref().and_then(|reference| {
                resolve_component_ref(world, reference, Some(id), QueryRootMode::WorldRoot)
            });
            let Some(upstream) = upstream else {
                invalidate_normalizer(world, id);
                continue;
            };
            let Some(amplitude) = world.get_component_by_id_as::<AmplitudeComponent>(upstream)
            else {
                invalidate_normalizer(world, id);
                continue;
            };
            // This first slice intentionally provisions only microphone input.
            let Some(source) = amplitude.resolved_source.filter(|source| {
                world
                    .get_component_by_id_as::<AudioInputComponent>(*source)
                    .is_some()
            }) else {
                invalidate_normalizer(world, id);
                continue;
            };
            if !normalization.enabled || !amplitude.enabled {
                invalidate_normalizer(world, id);
                continue;
            }
            let wanted = NormalizerState {
                source,
                generation: normalization.generation,
                upstream,
                upstream_generation: amplitude.generation,
            };
            if self.normalizers.get(&id).copied() != Some(wanted) {
                let component = world
                    .get_component_by_id_as_mut::<VolumeNormalizationComponent>(id)
                    .expect("component was just inspected");
                component.bump_generation(AmplitudeStatus::Pending);
                live_normalizers.insert(
                    id,
                    NormalizerState {
                        generation: component.generation,
                        ..wanted
                    },
                );
            } else {
                live_normalizers.insert(id, wanted);
            }
        }
        self.normalizers = live_normalizers;
    }

    pub(crate) fn input_consumers(&self, world: &World) -> Vec<InputAmplitudeConsumer> {
        let mut out: Vec<_> = self
            .consumers
            .iter()
            .filter_map(|(&observer, state)| {
                world.get_component_by_id_as::<AudioInputComponent>(state.source)?;
                let amplitude = world.get_component_by_id_as::<AmplitudeComponent>(observer)?;
                Some(InputAmplitudeConsumer {
                    observer,
                    source: state.source,
                    generation: state.generation,
                    window_sec: amplitude.window_sec,
                })
            })
            .collect();
        out.sort_by_key(|consumer| consumer.observer);
        out
    }

    pub(crate) fn input_normalization_consumers(
        &self,
        world: &World,
    ) -> Vec<InputNormalizationConsumer> {
        let mut out: Vec<_> = self
            .normalizers
            .iter()
            .filter_map(|(&normalizer, state)| {
                world.get_component_by_id_as::<AudioInputComponent>(state.source)?;
                let component =
                    world.get_component_by_id_as::<VolumeNormalizationComponent>(normalizer)?;
                Some(InputNormalizationConsumer {
                    normalizer,
                    source: state.source,
                    generation: state.generation,
                    // The upstream's requested window is the source analysis
                    // contract the normalizer wraps.
                    window_sec: world
                        .get_component_by_id_as::<AmplitudeComponent>(state.upstream)?
                        .window_sec,
                    policy: component.live_policy(),
                })
            })
            .collect();
        out.sort_by_key(|consumer| consumer.normalizer);
        out
    }

    pub(crate) fn invalidate_source(&mut self, world: &mut World, source: ComponentId) {
        let observers: Vec<_> = self
            .consumers
            .iter()
            .filter_map(|(&observer, state)| (state.source == source).then_some(observer))
            .collect();
        for observer in observers {
            if let Some(amplitude) =
                world.get_component_by_id_as_mut::<AmplitudeComponent>(observer)
            {
                amplitude.bump_generation(AmplitudeStatus::Invalid);
            }
            self.consumers.remove(&observer);
        }
        let normalizers: Vec<_> = self
            .normalizers
            .iter()
            .filter_map(|(&normalizer, state)| (state.source == source).then_some(normalizer))
            .collect();
        for normalizer in normalizers {
            invalidate_normalizer(world, normalizer);
            self.normalizers.remove(&normalizer);
        }
    }
}

fn invalidate_normalizer(world: &mut World, id: ComponentId) {
    if let Some(component) = world.get_component_by_id_as_mut::<VolumeNormalizationComponent>(id)
        && component.retained.status != AmplitudeStatus::Invalid
    {
        component.bump_generation(AmplitudeStatus::Invalid);
    }
}

fn is_audio_source(world: &World, id: ComponentId) -> bool {
    world
        .get_component_by_id_as::<AudioInputComponent>(id)
        .is_some()
        || world
            .get_component_by_id_as::<AudioClipComponent>(id)
            .is_some()
        || world
            .get_component_by_id_as::<AudioOscillatorComponent>(id)
            .is_some()
}

fn is_audio_source_enabled(world: &World, id: ComponentId) -> bool {
    world
        .get_component_by_id_as::<AudioInputComponent>(id)
        .map_or_else(|| is_audio_source(world, id), |input| input.enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{
        AmplitudeComponent, AudioInputComponent, ComponentRef, VolumeNormalizationComponent,
        VolumeNormalizationReason,
    };

    #[test]
    fn retains_only_current_generation_from_resolved_source() {
        let mut world = World::default();
        let source = world.add_component(AudioInputComponent::new());
        let source_guid = world.get_component_record(source).unwrap().guid;
        let observer = world.add_component(
            AmplitudeComponent::rolling_window(0.25)
                .unwrap()
                .with_source(ComponentRef::Guid(source_guid)),
        );
        let mut system = AmplitudeSystem::new();
        system.tick(&mut world);
        let generation = world
            .get_component_by_id_as::<AmplitudeComponent>(observer)
            .unwrap()
            .generation;
        system.submit_snapshot(AmplitudeSnapshot {
            observer,
            source,
            sample: AmplitudeSample {
                generation,
                sequence: 1,
                timestamp_sec: 1.0,
                valid_frames: 32,
                rms: 0.5,
                peak: 0.75,
                status: AmplitudeStatus::Live,
            },
        });
        system.tick(&mut world);
        assert_eq!(
            world
                .get_component_by_id_as::<AmplitudeComponent>(observer)
                .unwrap()
                .retained
                .rms,
            0.5
        );
    }

    #[test]
    fn neutral_snapshot_and_disabled_input_clear_retained_state() {
        let mut world = World::default();
        let source = world.add_component(AudioInputComponent::new());
        let source_guid = world.get_component_record(source).unwrap().guid;
        let observer = world.add_component(
            AmplitudeComponent::rolling_window(0.1)
                .unwrap()
                .with_source(ComponentRef::Guid(source_guid)),
        );
        let mut system = AmplitudeSystem::new();
        system.tick(&mut world);
        let generation = world
            .get_component_by_id_as::<AmplitudeComponent>(observer)
            .unwrap()
            .generation;
        system.submit_snapshot(AmplitudeSnapshot {
            observer,
            source,
            sample: AmplitudeSample {
                generation,
                sequence: 1,
                timestamp_sec: 1.0,
                valid_frames: 32,
                rms: 0.0,
                peak: 0.0,
                status: AmplitudeStatus::Neutral,
            },
        });
        system.tick(&mut world);
        assert_eq!(
            world
                .get_component_by_id_as::<AmplitudeComponent>(observer)
                .unwrap()
                .retained
                .status,
            AmplitudeStatus::Neutral,
        );

        world
            .get_component_by_id_as_mut::<AudioInputComponent>(source)
            .unwrap()
            .enabled = false;
        system.tick(&mut world);
        assert_eq!(
            world
                .get_component_by_id_as::<AmplitudeComponent>(observer)
                .unwrap()
                .retained
                .status,
            AmplitudeStatus::Invalid,
        );
    }

    #[test]
    fn normalized_input_consumer_retains_only_its_current_generation() {
        let mut world = World::default();
        let source = world.add_component(AudioInputComponent::new());
        let source_guid = world.get_component_record(source).unwrap().guid;
        let raw = world.add_component(
            AmplitudeComponent::rolling_window(0.08)
                .unwrap()
                .with_source(ComponentRef::Guid(source_guid)),
        );
        let raw_guid = world.get_component_record(raw).unwrap().guid;
        let normalizer = world.add_component(VolumeNormalizationComponent::from(
            ComponentRef::Guid(raw_guid),
        ));
        let mut system = AmplitudeSystem::new();
        system.tick(&mut world);
        let consumer = system.input_normalization_consumers(&world).pop().unwrap();
        assert_eq!(consumer.source, source);

        system.submit_normalized_snapshot(VolumeNormalizationSnapshot {
            normalizer,
            source,
            sample: AmplitudeSample {
                generation: consumer.generation,
                sequence: 1,
                timestamp_sec: 1.0,
                valid_frames: 32,
                rms: 0.04,
                peak: 0.2,
                status: AmplitudeStatus::Live,
            },
            gain_db: 6.0,
            reason: VolumeNormalizationReason::Raising,
        });
        system.tick(&mut world);
        let retained = world
            .get_component_by_id_as::<VolumeNormalizationComponent>(normalizer)
            .unwrap();
        assert_eq!(retained.retained.rms, 0.04);
        assert_eq!(retained.current_gain_db, 6.0);

        world
            .get_component_by_id_as_mut::<VolumeNormalizationComponent>(normalizer)
            .unwrap()
            .bump_generation(AmplitudeStatus::Pending);
        system.submit_normalized_snapshot(VolumeNormalizationSnapshot {
            normalizer,
            source,
            sample: AmplitudeSample {
                generation: consumer.generation,
                sequence: 2,
                timestamp_sec: 2.0,
                valid_frames: 32,
                rms: 0.9,
                peak: 0.9,
                status: AmplitudeStatus::Live,
            },
            gain_db: 0.0,
            reason: VolumeNormalizationReason::Holding,
        });
        system.tick(&mut world);
        assert_eq!(
            world
                .get_component_by_id_as::<VolumeNormalizationComponent>(normalizer)
                .unwrap()
                .retained
                .status,
            AmplitudeStatus::Pending,
        );
    }
}
