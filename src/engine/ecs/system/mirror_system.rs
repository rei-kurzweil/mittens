use crate::engine::ecs::component::{
    BoundsComponent, MirrorComponent, RenderableComponent, TextureComponent, TransformComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::primitives::{InstanceHandle, MaterialHandle};
use crate::engine::graphics::visual_world::VisualMirror;
use crate::engine::graphics::{CameraData, CameraTarget, VisualCamera, VisualWorld};
use crate::engine::user_input::InputState;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct MirrorSystem {
    pending_texture_registrations: Vec<ComponentId>,
}

impl MirrorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_pending_texture_registrations(&mut self) -> Vec<ComponentId> {
        std::mem::take(&mut self.pending_texture_registrations)
    }

    fn invert_affine_transform(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let c0 = [m[0][0], m[0][1], m[0][2]];
        let c1 = [m[1][0], m[1][1], m[1][2]];
        let c2 = [m[2][0], m[2][1], m[2][2]];

        let a00 = c0[0];
        let a10 = c0[1];
        let a20 = c0[2];
        let a01 = c1[0];
        let a11 = c1[1];
        let a21 = c1[2];
        let a02 = c2[0];
        let a12 = c2[1];
        let a22 = c2[2];

        let det = a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20)
            + a02 * (a10 * a21 - a11 * a20);
        if det.abs() < 1e-8 {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        }
        let inv_det = 1.0 / det;

        let inv00 = (a11 * a22 - a12 * a21) * inv_det;
        let inv01 = (a02 * a21 - a01 * a22) * inv_det;
        let inv02 = (a01 * a12 - a02 * a11) * inv_det;

        let inv10 = (a12 * a20 - a10 * a22) * inv_det;
        let inv11 = (a00 * a22 - a02 * a20) * inv_det;
        let inv12 = (a02 * a10 - a00 * a12) * inv_det;

        let inv20 = (a10 * a21 - a11 * a20) * inv_det;
        let inv21 = (a01 * a20 - a00 * a21) * inv_det;
        let inv22 = (a00 * a11 - a01 * a10) * inv_det;

        let tx = m[3][0];
        let ty = m[3][1];
        let tz = m[3][2];

        let itx = -(inv00 * tx + inv01 * ty + inv02 * tz);
        let ity = -(inv10 * tx + inv11 * ty + inv12 * tz);
        let itz = -(inv20 * tx + inv21 * ty + inv22 * tz);

        [
            [inv00, inv10, inv20, 0.0],
            [inv01, inv11, inv21, 0.0],
            [inv02, inv12, inv22, 0.0],
            [itx, ity, itz, 1.0],
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
        let mut pending_texture_registrations = HashSet::new();

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
                    let camera_world = eye_data.transform.matrix_world;
                    let cam_pos = [
                        camera_world[3][0],
                        camera_world[3][1],
                        camera_world[3][2],
                    ];
                    let cam_right = [camera_world[0][0], camera_world[0][1], camera_world[0][2]];
                    let cam_up = [camera_world[1][0], camera_world[1][1], camera_world[1][2]];
                    let cam_back = [camera_world[2][0], camera_world[2][1], camera_world[2][2]];

                    let ref_pos = Self::reflect_pos(cam_pos, plane_pos, plane_normal);
                    let ref_right = Self::reflect_dir(cam_right, plane_normal);
                    let ref_up = Self::reflect_dir(cam_up, plane_normal);
                    let ref_back = Self::reflect_dir(cam_back, plane_normal);

                    let ref_world = [
                        [ref_right[0], ref_right[1], ref_right[2], 0.0],
                        [ref_up[0], ref_up[1], ref_up[2], 0.0],
                        [ref_back[0], ref_back[1], ref_back[2], 0.0],
                        [ref_pos[0], ref_pos[1], ref_pos[2], 1.0],
                    ];
                    let ref_view = Self::invert_affine_transform(&ref_world);

                    let mut reflected_transform = eye_data.transform;
                    reflected_transform.matrix_world = ref_world;
                    reflected_transform.model = ref_world;
                    reflected_transform.translation = ref_pos;

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
                        transform: reflected_transform,
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
                if let Some(handle) = renderable.handle {
                    let _ = visuals.update_material(handle, MaterialHandle::MIRROR);
                }
                
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
                    pending_texture_registrations.insert(t_cid);
                } else {
                    let new_tex_id = world.add_component(TextureComponent::render_image(mirror_key));
                    let _ = world.add_child(renderable_cid, new_tex_id);
                    pending_texture_registrations.insert(new_tex_id);
                }
            }
        }

        self.pending_texture_registrations
            .extend(pending_texture_registrations);
    }
}
