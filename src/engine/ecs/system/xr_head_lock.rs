//! Holding an XR head at a target transform (a downed player's view from
//! their body), with a share of the headset's motion since the lock let
//! through so the view still feels like a head. The eye cameras and hands
//! follow the driven head, so this one pose moves the whole view.
use crate::engine::ecs::component::{HeadLock, InputXRComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math;

/// The locked head's world translation and rotation, or `None` when
/// `input` isn't locked or its target is gone. `head` is this frame's
/// tracking-space head pose; the first locked frame records it as the
/// anchor.
pub(crate) fn locked_head_pose(
    world: &mut World,
    input: ComponentId,
    head: [[f32; 4]; 4],
) -> Option<([f32; 3], [f32; 4])> {
    let head = ([head[3][0], head[3][1], head[3][2]], math::mat_to_quat(head));
    let lock = world
        .get_component_by_id_as_mut::<InputXRComponent>(input)?
        .head_lock
        .as_mut()?;
    let anchor = *lock.anchor.get_or_insert(head);
    let lock = *lock;
    let target = TransformSystem::world_model(world, lock.target)?;
    Some(follow(
        ([target[3][0], target[3][1], target[3][2]], math::mat_to_quat(target)),
        anchor,
        head,
        lock.translation_follow,
        lock.rotation_follow,
    ))
}

/// `target` moved by the scaled motion from `anchor` to `head`, measured in
/// the anchor's own frame so turning your head turns the view the same way
/// whichever way the target faces.
pub(crate) fn follow(
    target: ([f32; 3], [f32; 4]),
    anchor: ([f32; 3], [f32; 4]),
    head: ([f32; 3], [f32; 4]),
    translation_follow: f32,
    rotation_follow: f32,
) -> ([f32; 3], [f32; 4]) {
    let inverse = math::quat_conjugate(anchor.1);
    let turn = math::quat_mul(inverse, head.1);
    let shift = math::quat_rotate_vec3(inverse, math::vec3_sub(head.0, anchor.0));
    let turn = math::quat_nlerp([0.0, 0.0, 0.0, 1.0], turn, rotation_follow.clamp(0.0, 1.0));
    let shift = math::vec3_scale(shift, translation_follow.clamp(0.0, 1.0));
    (
        math::vec3_add(target.0, math::quat_rotate_vec3(target.1, shift)),
        math::quat_normalize(math::quat_mul(target.1, turn)),
    )
}

pub(crate) fn lock(world: &mut World, input: ComponentId, lock: Option<HeadLock>) -> Result<(), String> {
    let config = world
        .get_component_by_id_as_mut::<InputXRComponent>(input)
        .ok_or("not an InputXR component")?;
    config.head_lock = lock;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::follow;
    use crate::utils::math;

    fn close(a: [f32; 3], b: [f32; 3]) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-4, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn a_frozen_lock_stays_at_the_target_and_a_full_one_follows_the_head() {
        let target = ([1.0, 0.2, -3.0], math::quat_from_axis_angle([1.0, 0.0, 0.0], 1.2));
        let anchor = ([0.0, 1.6, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let head = ([0.3, 1.5, -0.2], math::quat_rotation_y(0.8));
        let frozen = follow(target, anchor, head, 0.0, 0.0);
        close(frozen.0, target.0);
        assert!(math::quat_mul(math::quat_conjugate(target.1), frozen.1)[3].abs() > 0.9999);
        // Followed fully, the target moves exactly as the head moved from the anchor.
        let full = follow(target, anchor, head, 1.0, 1.0);
        let expected = math::quat_mul(target.1, head.1);
        let dot: f32 = (0..4).map(|i| full.1[i] * expected[i]).sum();
        assert!(dot.abs() > 0.9999);
        close(full.0, math::vec3_add(target.0, math::quat_rotate_vec3(target.1, [0.3, -0.1, -0.2])));
    }

    #[test]
    fn a_quarter_follow_lets_through_a_quarter_of_the_turn() {
        let target = ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
        let anchor = ([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
        let head = ([0.0; 3], math::quat_rotation_y(0.8));
        let (_, turned) = follow(target, anchor, head, 0.0, 0.25);
        let (axis, angle) = math::quat_to_axis_angle(turned);
        assert!((angle - 0.2).abs() < 0.01, "{angle}");
        assert!(axis[1] > 0.99);
    }
}
