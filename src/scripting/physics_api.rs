//! `Physics.raycast(origin, direction, max_distance)`: the nearest solid
//! collision zone along a ray, for line-of-sight checks in scripts (an NPC's
//! shot blocked by cover). Returns `{ distance, point }` or null. Tests every
//! enabled, non-sensor collidable's zone in its own frame (triangle meshes of
//! imported and cut parts; boxes; spheres; capsules as their box), ignoring
//! zones the ray starts inside (meshes: surfaces behind it), so a shooter's
//! own body never blocks its shot. One query walks the collidables once; call
//! it per shot, not per frame.
use super::MittensHost;
use crate::engine::ecs::system::world_raycast::raycast;
use crate::scripting::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::{ValueSignature as Sig, ValueType as T};
    builder.namespace("Physics", |namespace| {
        namespace.api(
            "raycast",
            Sig::new(vec![T::Array, T::Array, T::F64], T::Any),
            MittensBinding::Api(MittensApi::PhysicsRaycast),
        );
    });
}

/// Nearest entry distance of a ray into the zones, if any.
impl MittensHost<'_, '_> {
    pub(super) fn invoke_physics_raycast(
        &mut self,
        args: &[mms::TransportValue],
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as S, TransportValue as V};
        fn vec3(value: &V) -> Option<[f32; 3]> {
            let V::Array(items) = value else { return None };
            let numbers: Vec<f32> = items
                .iter()
                .filter_map(|item| match item {
                    V::Number(n) => Some(*n as f32),
                    _ => None,
                })
                .collect();
            (numbers.len() == 3 && items.len() == 3).then(|| [numbers[0], numbers[1], numbers[2]])
        }
        let (Some(origin), Some(direction), Some(V::Number(max))) = (
            args.first().and_then(vec3),
            args.get(1).and_then(vec3),
            args.get(2),
        ) else {
            return Err(mms::HostError::failure(
                "Physics.raycast",
                "expected origin [x,y,z], direction [x,y,z] and a max distance",
            ));
        };
        let Some(distance) = raycast(self.world, origin, direction, *max as f32) else {
            return Ok(S::Transport(V::Null));
        };
        let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
        let point: Vec<V> = (0..3)
            .map(|i| V::Number((origin[i] + direction[i] / length * distance) as f64))
            .collect();
        Ok(S::Transport(V::Table(vec![
            ("distance".into(), V::Number(distance as f64)),
            ("point".into(), V::Array(point)),
        ])))
    }
}
