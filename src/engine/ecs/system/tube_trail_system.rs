//! Connected world-space trails posed after the source transform settles.
use std::collections::{HashMap, VecDeque};

use super::TransformSystem;
use super::particle_system::{EffectVisual, effect_material};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::mesh::MeshFactory;
use crate::engine::graphics::primitives::CpuMeshHandle;
use crate::engine::graphics::{
    CpuMesh, RenderAssets, RenderUploader, TransformMatrix, VisualWorld,
};
use crate::utils::math::{
    mat4_identity, mat4_mul_vec4, vec3_cross, vec3_dot, vec3_len, vec3_normalize, vec3_scale,
    vec3_sub,
};

const SEGMENTS: usize = 20;
const SIDES: usize = 12;
const RINGS: usize = SEGMENTS + 1;
const MAX_SAMPLES: usize = 128;
// The rocket's secondary wave makes 23 cycles across 10 primary cycles.
// Wrapping at this period keeps both waves continuous.
const FLOW_PERIOD: f64 = 10.0;

#[derive(Debug, Clone, Copy)]
pub struct TubeTrail {
    pub material: ComponentId,
    pub offset: [f32; 3],
    pub speed: f32,
    pub lifetime: f32,
    pub source_radius: f32,
    pub tail_radius: f32,
    pub opacity: f32,
    pub edge_wobble: f32,
    pub flow_wavelength: Option<f32>,
    pub stop_fade: f32,
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    time: f64,
    position: [f32; 3],
    velocity: [f32; 3],
}

#[derive(Debug)]
struct Trail {
    config: TubeTrail,
    samples: VecDeque<Sample>,
    bones: [TransformMatrix; RINGS],
    model: TransformMatrix,
    visual: EffectVisual,
    time: f64,
    flow_cycles: f64,
    fade: f32,
    emitting: bool,
}

impl Trail {
    fn new(config: TubeTrail) -> Self {
        Self {
            config,
            samples: VecDeque::with_capacity(MAX_SAMPLES),
            bones: [mat4_identity(); RINGS],
            model: mat4_identity(),
            visual: EffectVisual::default(),
            time: 0.0,
            flow_cycles: 0.0,
            fade: 1.0,
            emitting: true,
        }
    }

    fn update(&mut self, model: TransformMatrix, dt: f32) {
        self.time += f64::from(dt);
        if let Some(wavelength) = self.config.flow_wavelength {
            let cycles = f64::from(self.config.speed) * f64::from(dt) / f64::from(wavelength);
            self.flow_cycles = (self.flow_cycles + cycles).rem_euclid(FLOW_PERIOD);
        }
        let [x, y, z] = self.config.offset;
        let [x, y, z, _] = mat4_mul_vec4(model, [x, y, z, 1.0]);
        let position = [x, y, z];
        self.model[3] = [x, y, z, 1.0];
        let forward = vec3_normalize([-model[2][0], -model[2][1], -model[2][2]]);
        let sample = Sample {
            time: self.time,
            position,
            velocity: vec3_scale(forward, self.config.speed),
        };
        if self
            .samples
            .back()
            .is_some_and(|previous| vec3_len(vec3_sub(previous.position, position)) > 2.0)
        {
            self.samples.clear();
        }
        if self
            .samples
            .back()
            .is_some_and(|prev| prev.time == sample.time)
        {
            self.samples.pop_back();
        }
        if self.samples.len() == MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
        let cutoff = self.time - f64::from(self.config.lifetime);
        while self.samples.len() > 2 && self.samples[1].time < cutoff {
            self.samples.pop_front();
        }

        let centers: [[f32; 3]; RINGS] = std::array::from_fn(|ring| {
            let age = self.config.lifetime * ring as f32 / SEGMENTS as f32;
            self.position_at_age(age)
        });
        let mut right = [model[0][0], model[0][1], model[0][2]];
        for (ring, &center) in centers.iter().enumerate() {
            let t = ring as f32 / SEGMENTS as f32;
            let delta = vec3_sub(
                centers[(ring + 1).min(SEGMENTS)],
                centers[ring.saturating_sub(1)],
            );
            let tangent = if vec3_len(delta) > 0.00001 {
                vec3_normalize(delta)
            } else {
                forward
            };
            // Project the previous frame around the new tangent to keep twist continuous.
            right = vec3_sub(right, vec3_scale(tangent, vec3_dot(right, tangent)));
            if vec3_len(right) < 0.01 {
                let fallback = if tangent[1].abs() < 0.9 {
                    [0.0, 1.0, 0.0]
                } else {
                    [1.0, 0.0, 0.0]
                };
                right = vec3_cross(tangent, fallback);
            }
            right = vec3_normalize(right);
            let around = vec3_cross(right, tangent);
            let base_radius = self.config.source_radius
                + (self.config.tail_radius - self.config.source_radius) * t.sqrt();
            let phase = self.config.flow_wavelength.map_or_else(
                || t * 17.0 - (self.time % 1000.0) as f32 * 5.0,
                |wavelength| {
                    let cycles = self.config.speed * self.config.lifetime / wavelength;
                    std::f32::consts::TAU * (t * cycles - self.flow_cycles as f32)
                },
            );
            let swell = 1.0 + self.config.edge_wobble * t * phase.sin();
            let radius = (base_radius * swell * cap_radius(t)).max(0.0001);
            let along = vec3_scale(tangent, radius);
            let center = vec3_sub(vec3_sub(center, position), vec3_scale(along, t));
            let x = vec3_scale(right, radius);
            let z = vec3_scale(around, radius);
            self.bones[ring] = [
                [x[0], x[1], x[2], 0.0],
                [along[0], along[1], along[2], 0.0],
                [z[0], z[1], z[2], 0.0],
                [center[0], center[1], center[2], 1.0],
            ];
        }
    }

    fn position_at_age(&self, age: f32) -> [f32; 3] {
        let target = self.time - f64::from(age);
        let later = self.samples.partition_point(|sample| sample.time < target);
        let a = self.samples[later.saturating_sub(1)];
        let b = self.samples[later.min(self.samples.len() - 1)];
        let mix = if b.time > a.time {
            ((target - a.time) / (b.time - a.time)).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        std::array::from_fn(|axis| {
            a.position[axis]
                + (b.position[axis] - a.position[axis]) * mix
                + age * (a.velocity[axis] + (b.velocity[axis] - a.velocity[axis]) * mix)
        })
    }

    fn instance_color(&self) -> [f32; 4] {
        let alpha = self.config.opacity * self.fade;
        match self.config.flow_wavelength {
            // An opted-in trail shader reads primary cycles from R and G.
            // Alpha still routes the instance through the transparent pass.
            Some(wavelength) => [
                (self.flow_cycles / FLOW_PERIOD) as f32,
                self.config.speed * self.config.lifetime / wavelength,
                1.0,
                alpha,
            ],
            None => [1.0, 1.0, 1.0, alpha],
        }
    }
}

fn cap(t: f32) -> f32 {
    if t < 0.05 {
        t / 0.05 - 1.0
    } else {
        ((t - 0.8) / 0.2).clamp(0.0, 1.0)
    }
}
fn cap_radius(t: f32) -> f32 {
    (1.0 - cap(t).powi(2)).max(0.0).sqrt()
}

fn tube_mesh() -> CpuMesh {
    let mut mesh = MeshFactory::helix_tube(0.0, 1.0, 1.0, 0.0, 0.0, SEGMENTS as u32, SIDES as u32)
        .expect("fixed tube dimensions");
    let mut joints = Vec::with_capacity(mesh.vertices.len());
    for (index, vertex) in mesh.vertices.iter_mut().enumerate() {
        let t = vertex.uv[0];
        let radial = cap_radius(t);
        vertex.normal = [vertex.normal[0] * radial, cap(t), vertex.normal[2] * radial];
        joints.push([(index / (SIDES + 1)) as u16, 0, 0, 0]);
    }
    let count = joints.len();
    mesh.with_skinning(joints, vec![[1.0, 0.0, 0.0, 0.0]; count])
}

#[derive(Debug, Default)]
pub(crate) struct TubeTrailSystem {
    trails: HashMap<ComponentId, Trail>,
    mesh: Option<CpuMeshHandle>,
}

impl TubeTrailSystem {
    pub(crate) fn set_trail(&mut self, owner: ComponentId, config: Option<TubeTrail>) {
        if let Some(config) = config {
            let trail = self
                .trails
                .entry(owner)
                .or_insert_with(|| Trail::new(config));
            if !trail.emitting {
                trail.samples.clear();
                trail.fade = 1.0;
                trail.emitting = true;
            }
            trail.config = config;
        } else if let Some(trail) = self.trails.get_mut(&owner) {
            trail.emitting = false;
        }
    }

    pub(super) fn tick(&mut self, world: &World, visuals: &mut VisualWorld, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.trails.retain(|&owner, trail| {
            if trail.emitting {
                if let Some(model) = TransformSystem::world_model(world, owner) {
                    trail.update(model, dt);
                    return true;
                }
                trail.emitting = false;
            }
            trail.fade -= dt / trail.config.stop_fade;
            if trail.samples.is_empty() || trail.fade <= 0.0 {
                trail.visual.remove(visuals);
                return false;
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
        if !self.trails.values().any(|trail| !trail.samples.is_empty()) {
            return;
        }
        let cpu_mesh = *self
            .mesh
            .get_or_insert_with(|| assets.register_mesh(tube_mesh()));
        let Ok(mesh) = assets.gpu_mesh_handle(uploader, cpu_mesh) else {
            return;
        };
        for trail in self.trails.values_mut() {
            let material = effect_material(world, visuals, trail.config.material, true);
            if let (false, Some(material)) = (trail.samples.is_empty(), material) {
                let color = trail.instance_color();
                let handle = trail
                    .visual
                    .draw(visuals, mesh, material, trail.model, color);
                visuals.set_skin_matrices(handle, &trail.bones);
            } else {
                trail.visual.remove(visuals);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_tube_rings_follow_world_space_history_across_a_turn() {
        let mut trail = Trail::new(TubeTrail {
            material: ComponentId::default(),
            offset: [0.0; 3],
            speed: 2.0,
            lifetime: 0.5,
            source_radius: 0.05,
            tail_radius: 0.2,
            opacity: 0.7,
            edge_wobble: 0.1,
            flow_wavelength: Some(0.5),
            stop_fade: 0.3,
        });
        trail.update(mat4_identity(), 0.0);
        let mut turned = [
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.3, 0.0, 0.0, 1.0],
        ];
        trail.update(turned, 0.2);
        assert_eq!(trail.position_at_age(0.0), [0.3, 0.0, 0.0]);
        assert_eq!(trail.position_at_age(0.5), [0.0, 0.0, -1.0]);
        assert!(
            trail
                .bones
                .iter()
                .flatten()
                .flatten()
                .all(|v| v.is_finite())
        );

        turned[3][0] = 4.0;
        trail.update(turned, 0.02);
        assert_eq!(trail.samples.len(), 1);
        assert_eq!(trail.position_at_age(0.5), [3.0, 0.0, 0.0]);

        let previous_cycles = trail.flow_cycles;
        trail.config.speed = 4.0;
        trail.update(turned, 0.1);
        assert!((trail.flow_cycles - previous_cycles - 0.8).abs() < 1.0e-6);
        assert!((trail.instance_color()[1] - 4.0).abs() < 1.0e-6);

        trail.flow_cycles = FLOW_PERIOD - 0.2;
        trail.update(turned, 0.05);
        assert!((trail.flow_cycles - 0.2).abs() < 1.0e-6);
        assert!((trail.instance_color()[0] - 0.02).abs() < 1.0e-6);
    }
}
