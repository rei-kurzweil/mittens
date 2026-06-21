use crate::engine::ecs::component::{
    BoundsComponent, MirrorComponent, RenderableComponent, TextureComponent, TransformComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::primitives::{InstanceHandle, MaterialHandle};
use crate::engine::graphics::visual_world::{
    MirrorCaptureRequest, MirrorViewerFamily, VisualMirror,
};
use crate::engine::graphics::{CameraData, CameraTarget, VisualWorld};
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::HashSet;
use uuid::Uuid;
use winit::event::MouseButton;

const MIRROR_CLIP_BIAS_WORLD_UNITS: f32 = 0.01;
const MIRROR_ENABLE_OBLIQUE_CLIP_PLANE: bool = false;

#[derive(Debug, Default)]
pub struct MirrorSystem {
    pending_texture_registrations: Vec<ComponentId>,
    logged_debug_sample: bool,
}

impl MirrorSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_pending_texture_registrations(&mut self) -> Vec<ComponentId> {
        std::mem::take(&mut self.pending_texture_registrations)
    }

    fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
        let len2 = math::vec3_dot(v, v);
        if len2 <= 1e-12 {
            return None;
        }
        let inv_len = len2.sqrt().recip();
        Some([v[0] * inv_len, v[1] * inv_len, v[2] * inv_len])
    }

    fn mat4_mul_vec4(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
            m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
        ]
    }

    fn transform_plane_world_to_camera(
        view: [[f32; 4]; 4],
        plane_origin: [f32; 3],
        plane_normal_toward_camera: [f32; 3],
    ) -> Option<[f32; 4]> {
        let origin4 = Self::mat4_mul_vec4(
            view,
            [plane_origin[0], plane_origin[1], plane_origin[2], 1.0],
        );
        let normal4 = Self::mat4_mul_vec4(
            view,
            [
                plane_normal_toward_camera[0],
                plane_normal_toward_camera[1],
                plane_normal_toward_camera[2],
                0.0,
            ],
        );
        let normal = Self::normalize([normal4[0], normal4[1], normal4[2]])?;
        let origin = [origin4[0], origin4[1], origin4[2]];
        Some([
            normal[0],
            normal[1],
            normal[2],
            -math::vec3_dot(normal, origin),
        ])
    }

    fn projection_inverse_corner(proj: [[f32; 4]; 4], clip: [f32; 4]) -> Option<[f32; 4]> {
        let inv_proj = math::mat4_inverse(proj)?;
        Some(Self::mat4_mul_vec4(inv_proj, clip))
    }

    fn apply_oblique_near_plane_projection(
        proj: [[f32; 4]; 4],
        plane_camera: [f32; 4],
    ) -> Option<[[f32; 4]; 4]> {
        let clip_corner = [
            if plane_camera[0] >= 0.0 { 1.0 } else { -1.0 },
            if plane_camera[1] >= 0.0 { 1.0 } else { -1.0 },
            1.0,
            1.0,
        ];
        let q = Self::projection_inverse_corner(proj, clip_corner)?;
        let dot = plane_camera[0] * q[0]
            + plane_camera[1] * q[1]
            + plane_camera[2] * q[2]
            + plane_camera[3] * q[3];
        if dot.abs() <= 1e-6 {
            return None;
        }

        let scale = 1.0 / dot;
        let mut out = proj;
        out[0][2] = plane_camera[0] * scale;
        out[1][2] = plane_camera[1] * scale;
        out[2][2] = plane_camera[2] * scale;
        out[3][2] = plane_camera[3] * scale;
        Some(out)
    }

    fn mirror_local_basis(world_matrix: [[f32; 4]; 4]) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
        let x = Self::normalize([world_matrix[0][0], world_matrix[0][1], world_matrix[0][2]])?;
        let y = Self::normalize([world_matrix[1][0], world_matrix[1][1], world_matrix[1][2]])?;
        let z = Self::normalize([world_matrix[2][0], world_matrix[2][1], world_matrix[2][2]])?;
        Some((x, y, z))
    }

    fn build_camera_world_from_forward_up(
        position: [f32; 3],
        forward: [f32; 3],
        up_hint: [f32; 3],
    ) -> Option<[[f32; 4]; 4]> {
        let forward = Self::normalize(forward)?;
        let up_hint = Self::normalize(up_hint)?;
        let right = Self::normalize(math::vec3_cross(forward, up_hint)).or_else(|| {
            let fallback_up = if forward[1].abs() < 0.999 {
                [0.0, 1.0, 0.0]
            } else {
                [1.0, 0.0, 0.0]
            };
            Self::normalize(math::vec3_cross(forward, fallback_up))
        })?;
        let up = Self::normalize(math::vec3_cross(right, forward))?;
        let back = math::vec3_negate(forward);
        Some([
            [right[0], right[1], right[2], 0.0],
            [up[0], up[1], up[2], 0.0],
            [back[0], back[1], back[2], 0.0],
            [position[0], position[1], position[2], 1.0],
        ])
    }

    fn log_debug_sample_once(
        &mut self,
        force: bool,
        family: MirrorViewerFamily,
        view_index: usize,
        mirror_guid: &Uuid,
        plane_pos: [f32; 3],
        world_matrix: [[f32; 4]; 4],
        cam_pos: [f32; 3],
        ref_pos: [f32; 3],
        cam_forward: [f32; 3],
        ref_forward: [f32; 3],
        cam_up: [f32; 3],
        ref_up: [f32; 3],
    ) {
        if self.logged_debug_sample && !force {
            return;
        }
        let Some((mirror_x, mirror_y, mirror_z)) = Self::mirror_local_basis(world_matrix) else {
            return;
        };

        let source_from_plane = math::vec3_sub(cam_pos, plane_pos);
        let reflected_from_plane = math::vec3_sub(ref_pos, plane_pos);
        let source_local = [
            math::vec3_dot(source_from_plane, mirror_x),
            math::vec3_dot(source_from_plane, mirror_y),
            math::vec3_dot(source_from_plane, mirror_z),
        ];
        let reflected_local = [
            math::vec3_dot(reflected_from_plane, mirror_x),
            math::vec3_dot(reflected_from_plane, mirror_y),
            math::vec3_dot(reflected_from_plane, mirror_z),
        ];

        println!(
            "[mirror-debug] guid={mirror_guid} family={} view_index={view_index}",
            family.key_segment()
        );
        println!(
            "[mirror-debug] plane_pos={plane_pos:?} mirror_x={mirror_x:?} mirror_y={mirror_y:?} mirror_z={mirror_z:?}"
        );
        println!(
            "[mirror-debug] source_world_pos={cam_pos:?} source_local={source_local:?}"
        );
        println!(
            "[mirror-debug] reflected_world_pos={ref_pos:?} reflected_local={reflected_local:?}"
        );
        println!(
            "[mirror-debug] source_forward={cam_forward:?} reflected_forward={ref_forward:?}"
        );
        println!("[mirror-debug] source_up={cam_up:?} reflected_up={ref_up:?}");
        println!(
            "[mirror-debug] local_delta=[{:.6}, {:.6}, {:.6}] expected_z_negation_check=[sx-rx={:.6}, sy-ry={:.6}, sz+rz={:.6}]",
            reflected_local[0] - source_local[0],
            reflected_local[1] - source_local[1],
            reflected_local[2] - source_local[2],
            source_local[0] - reflected_local[0],
            source_local[1] - reflected_local[1],
            source_local[2] + reflected_local[2],
        );

        if !force {
            self.logged_debug_sample = true;
        }
    }
}

impl System for MirrorSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        _dt_sec: f32,
    ) {
        visuals.clear_mirrors();
        let mut pending_texture_registrations = HashSet::new();
        let force_debug_dump = input.mouse_pressed.contains(&MouseButton::Left);

        let mirror_cids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<MirrorComponent>(id)
                    .is_some()
            })
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
                if world
                    .get_component_by_id_as::<TransformComponent>(p)
                    .is_some()
                {
                    transform_cid = Some(p);
                    break;
                }
                cur = p;
            }
            let Some(transform_cid) = transform_cid else {
                continue;
            };
            let transform = world
                .get_component_by_id_as::<TransformComponent>(transform_cid)
                .unwrap();
            let world_matrix = transform.transform.matrix_world;

            // 2. Find parent renderable (the surface that should be reflective).
            let mut renderable_cid = None;
            if let Some(p) = world.parent_of(cid) {
                if world
                    .get_component_by_id_as::<RenderableComponent>(p)
                    .is_some()
                {
                    renderable_cid = Some(p);
                }
            }
            let Some(renderable_cid) = renderable_cid else {
                continue;
            };

            // 3. Find bounds (for aspect ratio).
            let bounds = world
                .get_component_by_id_as::<BoundsComponent>(renderable_cid)
                .map(|b| b.local)
                .or_else(|| {
                    // Try children of renderable.
                    world.children_of(renderable_cid).iter().find_map(|&ch| {
                        world
                            .get_component_by_id_as::<BoundsComponent>(ch)
                            .map(|b| b.local)
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
            // Start from the transform origin, then move the plane onto the renderable's
            // visible +Z face so thick mirror slabs reflect from the surface you actually see,
            // not from the center of the volume.
            let mut plane_pos = [world_matrix[3][0], world_matrix[3][1], world_matrix[3][2]];
            let plane_normal = {
                let n = [world_matrix[2][0], world_matrix[2][1], world_matrix[2][2]];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if len > 1e-6 {
                    [n[0] / len, n[1] / len, n[2] / len]
                } else {
                    [0.0, 0.0, 1.0]
                }
            };
            if let Some(b) = bounds {
                let z_axis_len = {
                    let z = [world_matrix[2][0], world_matrix[2][1], world_matrix[2][2]];
                    (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt()
                };
                if z_axis_len > 1e-6 {
                    plane_pos = math::vec3_add(
                        plane_pos,
                        math::vec3_scale(plane_normal, b.max[2] * z_axis_len),
                    );
                }
            }

            // 4. Derive reflected camera views for each active viewer family.
            let mut captures = Vec::new();
            let guid = world
                .get_component_node(cid)
                .map(|n| n.guid)
                .unwrap_or_default();
            let source_families = [
                (CameraTarget::Window, MirrorViewerFamily::Monoscopic),
                (CameraTarget::Xr, MirrorViewerFamily::Stereoscopic),
            ];

            for (source_target, family) in source_families {
                let Some(source_cam) = visuals.visual_camera(source_target) else {
                    continue;
                };
                if source_cam.eyes.is_empty() {
                    continue;
                }

                for (view_index, eye_data) in source_cam.eyes.iter().enumerate() {
                    let camera_world = eye_data.transform.matrix_world;
                    let cam_pos = [camera_world[3][0], camera_world[3][1], camera_world[3][2]];
                    let cam_up = [camera_world[1][0], camera_world[1][1], camera_world[1][2]];
                    let cam_back = [camera_world[2][0], camera_world[2][1], camera_world[2][2]];
                    let Some(cam_forward) = Self::normalize(math::vec3_negate(cam_back)) else {
                        continue;
                    };
                    let Some(cam_up) = Self::normalize(cam_up) else {
                        continue;
                    };

                    let ref_pos = math::vec3_reflect_point(cam_pos, plane_pos, plane_normal);
                    let Some(ref_forward) =
                        Self::normalize(math::vec3_reflect(cam_forward, plane_normal))
                    else {
                        continue;
                    };
                    let reflected_up_hint = math::vec3_reflect(cam_up, plane_normal);
                    let Some(ref_world) = Self::build_camera_world_from_forward_up(
                        ref_pos,
                        ref_forward,
                        reflected_up_hint,
                    ) else {
                        continue;
                    };
                    let ref_up = [ref_world[1][0], ref_world[1][1], ref_world[1][2]];

                    self.log_debug_sample_once(
                        force_debug_dump,
                        family,
                        view_index,
                        &guid,
                        plane_pos,
                        world_matrix,
                        cam_pos,
                        ref_pos,
                        cam_forward,
                        ref_forward,
                        cam_up,
                        ref_up,
                    );

                    let ref_view =
                        math::mat4_inverse(ref_world).unwrap_or_else(math::mat4_identity);

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

                    if MIRROR_ENABLE_OBLIQUE_CLIP_PLANE {
                        let plane_normal_toward_camera = if math::vec3_dot(plane_normal, ref_pos)
                            - math::vec3_dot(plane_normal, plane_pos)
                            < 0.0
                        {
                            plane_normal
                        } else {
                            math::vec3_negate(plane_normal)
                        };
                        let biased_plane_origin = math::vec3_add(
                            plane_pos,
                            math::vec3_scale(
                                plane_normal_toward_camera,
                                MIRROR_CLIP_BIAS_WORLD_UNITS,
                            ),
                        );
                        let plane_camera = Self::transform_plane_world_to_camera(
                            ref_view,
                            biased_plane_origin,
                            plane_normal_toward_camera,
                        );
                        if let Some(plane_camera) = plane_camera {
                            if let Some(oblique_proj) =
                                Self::apply_oblique_near_plane_projection(ref_proj, plane_camera)
                            {
                                ref_proj = oblique_proj;
                            }
                        }
                    }

                    captures.push(MirrorCaptureRequest {
                        family,
                        view_index,
                        camera: CameraData {
                            view: ref_view,
                            proj: ref_proj,
                            transform: reflected_transform,
                        },
                        target_key: format!(
                            "capture.mirror.{}.{}.{}.color",
                            guid,
                            family.key_segment(),
                            view_index
                        ),
                    });
                }
            }

            if captures.is_empty() {
                continue;
            }

            // 5. Register with VisualWorld.
            let source_instance = world
                .get_component_by_id_as::<RenderableComponent>(renderable_cid)
                .and_then(|r| r.handle)
                .unwrap_or(InstanceHandle(u32::MAX)); // renderer will handle null

            visuals.register_mirror(VisualMirror {
                mirror_component: cid,
                captures: captures.clone(),
                plane_origin: plane_pos,
                plane_normal,
                aspect_ratio: aspect,
                source_instance,
                resolution_scale: quality as f32 / 1024.0, // normalized scale? renderer decides
            });

            // 6. Override parent renderable's material and texture.
            if let Some(renderable) =
                world.get_component_by_id_as_mut::<RenderableComponent>(renderable_cid)
            {
                renderable.renderable.material = MaterialHandle::MIRROR;
                if let Some(handle) = renderable.handle {
                    let _ = visuals.update_material(handle, MaterialHandle::MIRROR);
                }

                // Ensure TextureComponent exists and points to mirror_key.
                let mut texture_cid = None;
                for &ch in world.children_of(renderable_cid) {
                    if world
                        .get_component_by_id_as::<TextureComponent>(ch)
                        .is_some()
                    {
                        texture_cid = Some(ch);
                        break;
                    }
                }

                if let Some(t_cid) = texture_cid {
                    let tex = world
                        .get_component_by_id_as_mut::<TextureComponent>(t_cid)
                        .unwrap();
                    tex.render_image = Some(format!("capture.mirror.{}.mono.0.color", guid));
                    pending_texture_registrations.insert(t_cid);
                } else {
                    let new_tex_id = world.add_component(TextureComponent::render_image(format!(
                        "capture.mirror.{}.mono.0.color",
                        guid
                    )));
                    let _ = world.add_child(renderable_cid, new_tex_id);
                    pending_texture_registrations.insert(new_tex_id);
                }
            }
        }

        self.pending_texture_registrations
            .extend(pending_texture_registrations);
    }
}

#[cfg(test)]
mod tests {
    use super::MirrorSystem;

    fn approx_eq(a: f32, b: f32) {
        assert!(
            (a - b).abs() <= 1e-4,
            "expected {a} ~= {b}, diff={}",
            (a - b).abs()
        );
    }

    fn clip_z(proj: [[f32; 4]; 4], v: [f32; 4]) -> f32 {
        MirrorSystem::mat4_mul_vec4(proj, v)[2]
    }

    #[test]
    fn transforms_world_plane_into_camera_space() {
        let view = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, -5.0, 1.0],
        ];

        let plane =
            MirrorSystem::transform_plane_world_to_camera(view, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0])
                .expect("plane");

        approx_eq(plane[0], 0.0);
        approx_eq(plane[1], 0.0);
        approx_eq(plane[2], 1.0);
        approx_eq(plane[3], 5.0);
    }

    #[test]
    fn oblique_projection_maps_clip_plane_to_near_plane() {
        let proj = crate::engine::ecs::system::camera_system::Camera3D::perspective_rh_zo(
            60.0f32.to_radians(),
            1.0,
            0.1,
            100.0,
        );
        let plane_camera = [0.0, 0.0, 1.0, 5.0];
        let oblique = MirrorSystem::apply_oblique_near_plane_projection(proj, plane_camera)
            .expect("oblique projection");

        approx_eq(clip_z(oblique, [0.0, 0.0, -5.0, 1.0]), 0.0);
        assert!(clip_z(oblique, [0.0, 0.0, -6.0, 1.0]) > 0.0);
        assert!(clip_z(oblique, [0.0, 0.0, -4.0, 1.0]) < 0.0);
    }

    #[test]
    fn oblique_projection_handles_flipped_plane_orientation() {
        let proj = crate::engine::ecs::system::camera_system::Camera3D::perspective_rh_zo(
            60.0f32.to_radians(),
            1.0,
            0.1,
            100.0,
        );
        let plane_camera = [0.0, 0.0, -1.0, -5.0];
        let oblique = MirrorSystem::apply_oblique_near_plane_projection(proj, plane_camera)
            .expect("oblique projection");

        approx_eq(clip_z(oblique, [0.0, 0.0, -5.0, 1.0]), 0.0);
        assert!(clip_z(oblique, [0.0, 0.0, -6.0, 1.0]) > 0.0);
        assert!(clip_z(oblique, [0.0, 0.0, -4.0, 1.0]) < 0.0);
    }
}
