//! Retained, world-space particles with a bounded pool per emitter.
use std::collections::HashMap;

use super::TransformSystem;
use super::material_system::MaterialSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::materials::MaterialInstanceId;
use crate::engine::graphics::mesh::MeshFactory;
use crate::engine::graphics::primitives::{CpuMeshHandle, InstanceHandle};
use crate::engine::graphics::{
    GpuRenderable, MaterialHandle, MeshHandle, RenderAssets, RenderUploader, Transform,
    TransformMatrix, VisualWorld,
};
use crate::utils::math;

const MAX_CAPACITY: usize = 64;

/// Initial conditions are authored in emitter-local space. Acceleration and
/// live particle positions are in world space, independent of emitter motion.
#[derive(Debug, Clone, Copy)]
pub struct ParticleEmission {
    pub material: ComponentId,
    pub capacity: usize,
    pub interval: f32,
    pub lifetime: f32,
    pub offset: [f32; 3],
    pub velocity: [f32; 3],
    pub acceleration: [f32; 3],
    pub spread: f32,
    pub diameter: f32,
    pub growth: f32,
    pub opacity: f32,
    pub fade_start: f32,
    pub colors: [[f32; 3]; 3],
    pub color_times: [f32; 2],
}

impl Default for ParticleEmission {
    fn default() -> Self {
        Self {
            material: ComponentId::default(),
            capacity: 16,
            interval: 0.08,
            lifetime: 0.6,
            offset: [0.0; 3],
            velocity: [0.0, 0.2, 0.0],
            acceleration: [0.0; 3],
            spread: 0.0,
            diameter: 0.08,
            growth: 3.0,
            opacity: 0.5,
            fade_start: 0.28,
            colors: [[1.0; 3]; 3],
            color_times: [0.5, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Particle {
    origin: [f32; 3],
    velocity: [f32; 3],
    age: f32,
    birth: ParticleEmission,
}

impl Particle {
    fn appearance(self) -> (TransformMatrix, [f32; 4]) {
        let age = (self.age / self.birth.lifetime).clamp(0.0, 1.0);
        let growth = smoothstep(0.0, 0.9, age);
        let diameter = self.birth.diameter * (1.0 + (self.birth.growth - 1.0) * growth);
        let center: [f32; 3] = std::array::from_fn(|i| {
            self.origin[i]
                + self.velocity[i] * self.age
                + 0.5 * self.birth.acceleration[i] * self.age * self.age
        });
        let model = [
            [diameter, 0.0, 0.0, 0.0],
            [0.0, diameter, 0.0, 0.0],
            [0.0, 0.0, diameter, 0.0],
            [center[0], center[1], center[2], 1.0],
        ];
        let [start, middle, end] = self.birth.colors;
        let [mid_time, end_time] = self.birth.color_times;
        let color = if age <= mid_time {
            mix(start, middle, smoothstep(0.0, mid_time, age))
        } else {
            mix(middle, end, smoothstep(mid_time, end_time, age))
        };
        let alpha = self.birth.opacity * (1.0 - smoothstep(self.birth.fade_start, 1.0, age));
        (model, [color[0], color[1], color[2], alpha])
    }
}

#[derive(Debug, Default)]
pub(super) struct EffectVisual {
    handle: Option<InstanceHandle>,
    visible: bool,
}

impl EffectVisual {
    pub(super) fn draw(
        &mut self,
        visuals: &mut VisualWorld,
        mesh: MeshHandle,
        (material, program): (MaterialInstanceId, MaterialHandle),
        model: TransformMatrix,
        color: [f32; 4],
    ) -> InstanceHandle {
        if let Some((handle, previous_material, previous_program)) =
            self.handle.and_then(|handle| {
                visuals.instance(handle).map(|instance| {
                    (
                        handle,
                        instance.material_instance,
                        instance.renderable.material,
                    )
                })
            })
        {
            visuals.update_model(handle, model);
            visuals.update_color(handle, color);
            if previous_program != program {
                visuals.update_material(handle, program);
            }
            if previous_material != Some(material) {
                visuals.bind_material_instance(handle, Some(material));
            }
        } else {
            let handle = visuals.register(
                ComponentId::default(),
                GpuRenderable::new(mesh, program),
                Transform {
                    model,
                    matrix_world: model,
                    ..Default::default()
                },
                color,
                1.0,
                false,
                false,
                false,
                false,
                false,
                0.0,
                None,
                3.0,
            );
            visuals.bind_material_instance(handle, Some(material));
            self.handle = Some(handle);
        }
        self.visible = true;
        self.handle.unwrap()
    }

    fn hide(&mut self, visuals: &mut VisualWorld) {
        if let Some(handle) = self.handle.filter(|_| self.visible) {
            // Keep the retained slot but collapse its coverage while unused.
            if let Some(instance) = visuals.instance(handle) {
                let mut model = instance.transform.matrix_world;
                model[0][0] = 0.00001;
                model[1][1] = 0.00001;
                model[2][2] = 0.00001;
                visuals.update_model(handle, model);
            }
            visuals.update_color(handle, [0.0; 4]);
        }
        self.visible = false;
    }

    pub(super) fn remove(&mut self, visuals: &mut VisualWorld) {
        if let Some(handle) = self.handle.take() {
            visuals.remove(handle);
        }
        self.visible = false;
    }
}

#[derive(Debug, Default)]
struct Slot {
    particle: Option<Particle>,
    visual: EffectVisual,
}

#[derive(Debug, Default)]
struct Emitter {
    config: Option<ParticleEmission>,
    slots: Vec<Slot>,
    previous_origin: Option<[f32; 3]>,
    until_next: f32,
    serial: u32,
    cursor: usize,
}

impl Emitter {
    fn configure(&mut self, config: Option<ParticleEmission>) {
        if let Some(settings) = config {
            let capacity = settings.capacity.clamp(1, MAX_CAPACITY);
            if self.slots.len() < capacity {
                self.slots.resize_with(capacity, Slot::default);
            }
        } else {
            self.previous_origin = None;
            self.until_next = 0.0;
        }
        self.config = config;
    }

    fn has_particles(&self) -> bool {
        self.slots.iter().any(|slot| slot.particle.is_some())
    }

    fn tick(&mut self, model: Option<TransformMatrix>, dt: f32) {
        for slot in &mut self.slots {
            if let Some(particle) = &mut slot.particle {
                particle.age += dt;
                if particle.age >= particle.birth.lifetime {
                    slot.particle = None;
                }
            }
        }
        let Some((model, config)) = model.zip(self.config) else {
            return;
        };
        let [x, y, z] = config.offset;
        let [x, y, z, _] = math::mat4_mul_vec4(model, [x, y, z, 1.0]);
        let origin = [x, y, z];
        let previous = self
            .previous_origin
            .replace(origin)
            .filter(|&previous| math::vec3_len(math::vec3_sub(origin, previous)) < 2.0)
            .unwrap_or(origin);
        if dt <= 0.0 {
            return;
        }

        let axes: [[f32; 3]; 3] =
            std::array::from_fn(|i| math::vec3_normalize([model[i][0], model[i][1], model[i][2]]));
        let velocity = std::array::from_fn(|i| {
            axes[0][i] * config.velocity[0]
                + axes[1][i] * config.velocity[1]
                + axes[2][i] * config.velocity[2]
        });
        let mut age = dt - self.until_next.min(config.interval);
        // A frame gap cannot produce an unbounded burst. Keep the newest births.
        let max_age = config
            .lifetime
            .min(config.interval * self.slots.len() as f32);
        if age >= max_age {
            age -= ((age - max_age) / config.interval).ceil() * config.interval;
        }
        for _ in 0..=self.slots.len() {
            if age < 0.0 {
                break;
            }
            if age < max_age {
                let start = mix(previous, origin, (1.0 - age / dt).clamp(0.0, 1.0));
                self.spawn(start, velocity, config, age);
            }
            age -= config.interval;
        }
        self.until_next = (-age).max(0.0);
    }

    fn spawn(&mut self, origin: [f32; 3], velocity: [f32; 3], config: ParticleEmission, age: f32) {
        let serial = self.serial;
        self.serial = self.serial.wrapping_add(1);
        let (sin, cos) = (serial as f32 * 2.399_963_1).sin_cos();
        let y = ((serial.wrapping_mul(37) % 101) as f32 / 50.0) - 1.0;
        let radial = (1.0 - y * y).sqrt();
        let spread = [radial * cos, y, radial * sin];
        let capacity = config.capacity.clamp(1, self.slots.len());
        let next = self.cursor % capacity;
        self.slots[next].particle = Some(Particle {
            origin,
            velocity: std::array::from_fn(|i| velocity[i] + spread[i] * config.spread),
            age,
            birth: config,
        });
        self.cursor = (next + 1) % capacity;
    }

    fn release_visuals(&mut self, visuals: &mut VisualWorld) {
        for slot in &mut self.slots {
            slot.visual.remove(visuals);
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParticleSystem {
    emitters: HashMap<ComponentId, Emitter>,
    sphere_mesh: Option<CpuMeshHandle>,
}

impl ParticleSystem {
    pub(crate) fn set_emission(&mut self, owner: ComponentId, config: Option<ParticleEmission>) {
        if config.is_some() {
            self.emitters.entry(owner).or_default().configure(config);
        } else if let Some(emitter) = self.emitters.get_mut(&owner) {
            emitter.configure(None);
        }
    }

    pub(super) fn tick(&mut self, world: &World, visuals: &mut VisualWorld, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.emitters.retain(|&owner, emitter| {
            let model = TransformSystem::world_model(world, owner);
            if model.is_none() {
                emitter.configure(None);
            }
            emitter.tick(model, dt);
            if !emitter.has_particles() {
                if emitter.config.is_none() {
                    emitter.release_visuals(visuals);
                    return false;
                }
                // Slow emitters keep their slots between bursts, including
                // frames when render preparation can skip the entire pool.
                for slot in &mut emitter.slots {
                    slot.visual.hide(visuals);
                }
            }
            true
        });
    }

    pub(super) fn prepare(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
    ) {
        if !self.emitters.values().any(Emitter::has_particles) {
            return;
        }
        let sphere = *self
            .sphere_mesh
            .get_or_insert_with(|| assets.register_mesh(MeshFactory::icosahedron(1, 1.0)));
        let Ok(mesh) = assets.gpu_mesh_handle(uploader, sphere) else {
            return;
        };
        for emitter in self.emitters.values_mut() {
            // The current profile usually supplies the material for every live slot.
            // A profile change can leave particles using an earlier material until they fade.
            let mut last_material = None;
            for slot in &mut emitter.slots {
                if let Some(particle) = slot.particle {
                    let source = particle.birth.material;
                    let material = match last_material {
                        Some((previous, material)) if previous == source => material,
                        _ => {
                            let material = effect_material(world, visuals, source, false);
                            last_material = Some((source, material));
                            material
                        }
                    };
                    if let Some(material) = material {
                        let (model, color) = particle.appearance();
                        slot.visual.draw(visuals, mesh, material, model, color);
                        continue;
                    }
                }
                slot.visual.hide(visuals);
            }
        }
    }
}

pub(super) fn effect_material(
    world: &World,
    visuals: &mut VisualWorld,
    source: ComponentId,
    deformed: bool,
) -> Option<(MaterialInstanceId, MaterialHandle)> {
    let material = visuals
        .materials
        .source_instance(source)
        .or_else(|| MaterialSystem::ensure_source(world, visuals, source).ok())?;
    let instance = visuals.materials.instance(material)?;
    let definition = visuals.materials.definition(instance.definition)?;
    let program = if deformed {
        definition.deformed_program?
    } else {
        definition.static_program
    };
    Some((material, program))
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_particles_preserve_birth_motion_and_drain_when_stopped() {
        let mut emitter = Emitter::default();
        let config = ParticleEmission {
            capacity: 4,
            interval: 0.1,
            lifetime: 0.25,
            offset: [2.0, 0.0, 0.0],
            velocity: [0.0, 2.0, 0.0],
            acceleration: [0.0, -4.0, 0.0],
            ..ParticleEmission::default()
        };
        let transform = Some(Transform::default().matrix_world);
        emitter.configure(Some(config));
        emitter.tick(transform, 0.05);
        let old_particle = emitter.slots[0].particle.unwrap();
        let (model, _) = old_particle.appearance();
        assert_eq!(model[3][0], 2.0);
        assert!((model[3][1] - 0.095).abs() < 0.0001);

        emitter.configure(Some(ParticleEmission {
            velocity: [0.0, 0.0, 3.0],
            ..config
        }));
        emitter.tick(transform, 0.06);
        assert_eq!(emitter.slots[0].particle.unwrap().velocity, [0.0, 2.0, 0.0]);
        assert_eq!(emitter.slots[1].particle.unwrap().velocity, [0.0, 0.0, 3.0]);

        emitter.configure(None);
        emitter.tick(transform, 0.25);
        assert!(!emitter.has_particles());
    }

    #[test]
    fn retained_puff_color_changes_keep_draw_topology() {
        let mut visuals = VisualWorld::new();
        let handle = visuals.register(
            ComponentId::default(),
            GpuRenderable::new(MeshHandle(0), MaterialHandle::TOON_MESH),
            Transform::default(),
            [1.0, 0.88, 0.30, 0.59],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        assert!(visuals.prepare_draw_cache());
        assert!(visuals.update_color(handle, [0.65, 0.65, 0.65, 0.21]));
        assert!(!visuals.prepare_draw_cache());
        assert!(visuals.update_color(handle, [0.65, 0.65, 0.65, 1.0]));
        assert!(visuals.prepare_draw_cache());
    }

    #[test]
    fn sparse_emitter_hides_an_expired_particle_between_births() {
        use crate::engine::ecs::component::TransformComponent;
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let mut visuals = VisualWorld::new();
        let mut particles = ParticleSystem::default();
        particles.set_emission(
            owner,
            Some(ParticleEmission {
                interval: 1.0,
                lifetime: 0.1,
                ..ParticleEmission::default()
            }),
        );
        particles.tick(&world, &mut visuals, 0.01);
        let handle = visuals.register(
            ComponentId::default(),
            GpuRenderable::new(MeshHandle(0), MaterialHandle::TOON_MESH),
            Transform::default(),
            [1.0, 1.0, 1.0, 0.5],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        particles.emitters.get_mut(&owner).unwrap().slots[0].visual = EffectVisual {
            handle: Some(handle),
            visible: true,
        };
        particles.tick(&world, &mut visuals, 0.2);
        assert_eq!(visuals.instance(handle).unwrap().color[3], 0.0);
        particles.set_emission(owner, None);
        particles.tick(&world, &mut visuals, 0.0);
        assert!(visuals.instance(handle).is_none());
    }
}
