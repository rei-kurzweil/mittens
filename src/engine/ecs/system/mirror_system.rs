use crate::engine::ecs::component::{
    BoundsComponent, MirrorComponent, RenderableComponent, TextureComponent, TransformComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::primitives::{InstanceHandle, MaterialHandle};
use crate::engine::graphics::visual_world::VisualMirror;
use crate::engine::graphics::{CameraData, CameraTarget, VisualCamera, VisualWorld};
use crate::engine::user_input::InputState;

#[derive(Debug, Default)]
pub struct MirrorSystem;

impl MirrorSystem {
    pub fn new() -> Self {
        Self
    }

    fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] =
                    a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
            }
        }
        out
    }

    fn mat4_invert(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
        // Simplified inversion for orthonormal (plus scale) matrices.
        // For reflection, it's still mostly orthonormal.
        // Let's use a more robust version if available, or just the basics.
        // Translation part: T' = -R^T * T
        let r0 = [m[0][0], m[1][0], m[2][0]];
        let r1 = [m[0][1], m[1][1], m[2][1]];
        let r2 = [m[0][2], m[1][2], m[2][2]];
        let t = [m[3][0], m[3][1], m[3][2]];

        let it = [
            -(r0[0] * t[0] + r0[1] * t[1] + r0[2] * t[2]),
            -(r1[0] * t[0] + r1[1] * t[1] + r1[2] * t[2]),
            -(r2[0] * t[0] + r2[1] * t[1] + r2[2] * t[2]),
        ];

        [
            [r0[0], r0[1], r0[2], 0.0],
            [r1[0], r1[1], r1[2], 0.0],
            [r2[0], r2[1], r2[2], 0.0],
            [it[0], it[1], it[2], 1.0],
        ]
    }

    fn reflect_pos(pos: [f32; 3], plane_pos: [f32; 3], plane_normal: [f32; 3]) -> [f32; 3] {
        let v = [pos[0] - plane_pos[0], pos[1] - plane_pos[1], pos[2] - plane_pos[2]];
        let dist = v[0] * plane_normal[0] + v[1] * plane_normal[1] + v[2] * plane_normal[2];
        [
            pos[0] - 2.0 * dist * plane_normal[0],
            pos[1] - 2.0 * dist * plane_normal[1],
            pos[2] - 2.0 * dist * plane_normal[2],
        ]
    }

    fn reflect_dir(dir: [f32; 3], plane_normal: [f32; 3]) -> [f32; 3] {
        let dist = dir[0] * plane_normal[0] + dir[1] * plane_normal[1] + dir[2] * plane_normal[2];
        [
            dir[0] - 2.0 * dist * plane_normal[0],
            dir[1] - 2.0 * dist * plane_normal[1],
            dir[2] - 2.0 * dist * plane_normal[2],
        ]
    }
}

impl System for MirrorSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        visuals.clear_mirrors();

        let mirror_cids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| world.get_component_by_id_as::<MirrorComponent>(id).is_some())
            .collect();

        for cid in mirror_cids {
            let quality = world
                .get_component_by_id_as::<MirrorComponent>(cid)
                .map(|m| m.quality)
                .unwrap_or(512);

            // 1. Find transform (nearest ancestor).
            let mut transform_cid = None;
            let mut cur = cid;
            while let Some(p) = world.parent_of(cur) {
                if world.get_component_by_id_as::<TransformComponent>(p).is_some() {
                    transform_cid = Some(p);
                    break;
                }
                cur = p;
            }
            let Some(transform_cid) = transform_cid else { continue };
            let transform = world.get_component_by_id_as::<TransformComponent>(transform_cid).unwrap();
            let world_matrix = transform.transform.matrix_world;

            // 2. Find parent renderable (the surface that should be reflective).
            let mut renderable_cid = None;
            if let Some(p) = world.parent_of(cid) {
                if world.get_component_by_id_as::<RenderableComponent>(p).is_some() {
                    renderable_cid = Some(p);
                }
            }
            let Some(renderable_cid) = renderable_cid else { continue };

            // 3. Find bounds (for aspect ratio).
            let bounds = world.get_component_by_id_as::<BoundsComponent>(renderable_cid)
                .map(|b| b.local)
                .or_else(|| {
                    // Try children of renderable.
                    world.children_of(renderable_cid).iter().find_map(|&ch| {
                        world.get_component_by_id_as::<BoundsComponent>(ch).map(|b| b.local)
                    })
                });

            // Derive aspect ratio from bounds (XY plane).
            let aspect = if let Some(b) = bounds {
                let w = (b.max[0] - b.min[0]).abs();
                let h = (b.max[1] - b.min[1]).abs();
                if h > 1e-6 { w / h } else { 1.0 }
            } else {
                1.0
            };

            // Mirror plane in world space.
            // Local origin: m[3]
            // Local normal (+Z): m[2]
            let plane_pos = [world_matrix[3][0], world_matrix[3][1], world_matrix[3][2]];
            let plane_normal = {
                let n = [world_matrix[2][0], world_matrix[2][1], world_matrix[2][2]];
                let len = (n[0]*n[0] + n[1]*n[1] + n[2]*n[2]).sqrt();
                if len > 1e-6 { [n[0]/len, n[1]/len, n[2]/len] } else { [0.0, 0.0, 1.0] }
            };

            // 4. Derive reflected camera views.
            // We reflect the viewer camera(s) (Window or XR).
            let mut derived_eyes = Vec::new();

            // Prefer active XR camera if available, otherwise Window.
            let source_target = if visuals.visual_camera(CameraTarget::Xr).map_or(false, |c| !c.eyes.is_empty()) {
                CameraTarget::Xr
            } else {
                CameraTarget::Window
            };

            if let Some(source_cam) = visuals.visual_camera(source_target) {
                for eye_data in &source_cam.eyes {
                    // View matrix = inverse(CameraWorldTransform)
                    // We need the camera's world position and basis.
                    let inv_view = Self::mat4_invert(eye_data.view);
                    let cam_pos = [inv_view[3][0], inv_view[3][1], inv_view[3][2]];
                    let cam_right = [inv_view[0][0], inv_view[0][1], inv_view[0][2]];
                    let cam_up = [inv_view[1][0], inv_view[1][1], inv_view[1][2]];
                    let cam_fwd = [inv_view[2][0], inv_view[2][1], inv_view[2][2]];

                    let ref_pos = Self::reflect_pos(cam_pos, plane_pos, plane_normal);
                    let ref_right = Self::reflect_dir(cam_right, plane_normal);
                    let ref_up = Self::reflect_dir(cam_up, plane_normal);
                    let ref_fwd = Self::reflect_dir(cam_fwd, plane_normal);

                    // Reconstruct reflected view matrix.
                    // New "forward" for the mirror camera should be pointing into the mirror?
                    // Standard reflection: if we look at the mirror, we see the reflected world.
                    // The reflected camera is "behind" the mirror looking "out".
                    
                    // Orthonormal basis: [ref_right, ref_up, ref_fwd]
                    let ref_inv_view = [
                        [ref_right[0], ref_right[1], ref_right[2], 0.0],
                        [ref_up[0], ref_up[1], ref_up[2], 0.0],
                        [ref_fwd[0], ref_fwd[1], ref_fwd[2], 0.0],
                        [ref_pos[0], ref_pos[1], ref_pos[2], 1.0],
                    ];
                    let ref_view = Self::mat4_invert(ref_inv_view);

                    // Projection: Adjust aspect ratio if it differs from source.
                    let mut ref_proj = eye_data.proj;
                    // proj[0][0] = 1 / (tan(fov/2) * aspect)
                    // We want to replace the aspect part.
                    // old_aspect = proj[1][1] / proj[0][0]
                    // new_proj[0][0] = proj[1][1] / new_aspect
                    if aspect > 1e-6 {
                        ref_proj[0][0] = ref_proj[1][1] / aspect;
                    }

                    derived_eyes.push(CameraData {
                        view: ref_view,
                        proj: ref_proj,
                        transform: Default::default(),
                    });
                }
            }

            if derived_eyes.is_empty() { continue; }

            let guid = world.get_component_node(cid).map(|n| n.guid).unwrap_or_default();
            let mirror_key = format!("capture.mirror.{}.color", guid);

            // 5. Register with VisualWorld.
            let source_instance = world.get_component_by_id_as::<RenderableComponent>(renderable_cid)
                .and_then(|r| r.handle)
                .unwrap_or(InstanceHandle(u32::MAX)); // renderer will handle null

            visuals.register_mirror(VisualMirror {
                mirror_component: cid,
                camera: VisualCamera {
                    target: CameraTarget::Window, // offscreen views are effectively ad-hoc window-like passes
                    eyes: derived_eyes,
                },
                target_key: mirror_key.clone(),
                source_instance,
                resolution_scale: quality as f32 / 1024.0, // normalized scale? renderer decides
            });

            // 6. Override parent renderable's material and texture.
            if let Some(renderable) = world.get_component_by_id_as_mut::<RenderableComponent>(renderable_cid) {
                renderable.renderable.material = MaterialHandle::MIRROR;
                
                // Ensure TextureComponent exists and points to mirror_key.
                let mut texture_cid = None;
                for &ch in world.children_of(renderable_cid) {
                    if world.get_component_by_id_as::<TextureComponent>(ch).is_some() {
                        texture_cid = Some(ch);
                        break;
                    }
                }

                if let Some(t_cid) = texture_cid {
                    let tex = world.get_component_by_id_as_mut::<TextureComponent>(t_cid).unwrap();
                    tex.render_image = Some(mirror_key);
                } else {
                    let mut tex = TextureComponent::render_image(mirror_key);
                    let new_tex_id = world.add_component(tex);
                    let _ = world.add_child(renderable_cid, new_tex_id);
                    // SystemWorld will pick this up on next frame's registration or we trigger it.
                }
            }
        }
    }
}
