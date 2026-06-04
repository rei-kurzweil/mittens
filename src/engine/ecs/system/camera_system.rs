use crate::engine::ecs::component::Camera3DComponent;
use crate::engine::ecs::component::CameraXRComponent;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::graphics::VisualWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraHandle(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct Camera3D {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy)]
enum AnyCamera {
    Camera3D(Camera3D),
    Camera2D(Camera2D),
}

impl Camera3D {
    pub fn identity() -> Self {
        Self {
            view: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Right-handed perspective projection matrix.
    ///
    /// Assumptions:
    /// - Column-major mat4 (matches how we pack instance matrices / GLSL default).
    /// - NDC depth range is Vulkan-style: z in [0, 1].
    pub fn perspective_rh_zo(
        fov_y_radians: f32,
        aspect: f32,
        z_near: f32,
        z_far: f32,
    ) -> [[f32; 4]; 4] {
        // Based on the standard RH, zero-to-one depth projection.
        // Maps camera forward -Z.
        let f = 1.0 / (0.5 * fov_y_radians).tan();
        let nf = 1.0 / (z_near - z_far);

        // Column-major:
        // [ f/aspect, 0,  0,                      0 ]
        // [ 0,        f,  0,                      0 ]
        // [ 0,        0,  z_far*nf,               -1 ]
        // [ 0,        0,  z_near*z_far*nf,         0 ]
        [
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, z_far * nf, -1.0],
            [0.0, 0.0, (z_near * z_far) * nf, 0.0],
        ]
    }
}

impl Camera2D {
    pub fn identity() -> Self {
        Self {
            view: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct CameraSystem {
    next_handle: u32,
    cameras: Vec<(CameraHandle, AnyCamera)>,
    camera2d_components: std::collections::HashMap<CameraHandle, ComponentId>,
    camera3d_components: std::collections::HashMap<CameraHandle, ComponentId>,
    pub active_window_camera: Option<CameraHandle>,
    pub active_xr_camera: Option<ComponentId>,

    // Track viewport changes for the no-camera fallback projection.
    last_viewport: Option<[f32; 2]>,
}

impl CameraSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a camera derived from the component tree.
    ///
    /// The newest registered camera becomes active.
    pub fn register_camera(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) -> CameraHandle {
        // Default 3D camera parameters.
        let (w, h) = {
            let vp = visuals.viewport();
            (vp[0], vp[1])
        };
        let aspect = if h > 0.0 { w / h } else { 1.0 };

        let (fov_y_deg, z_near, z_far) = world
            .get_component_by_id_as::<Camera3DComponent>(component)
            .map(|c| (c.fov_y_degrees, c.z_near, c.z_far))
            .unwrap_or((
                Camera3DComponent::DEFAULT_FOV_Y_DEGREES,
                Camera3DComponent::DEFAULT_Z_NEAR,
                Camera3DComponent::DEFAULT_Z_FAR,
            ));
        let proj = Camera3D::perspective_rh_zo(fov_y_deg.to_radians(), aspect, z_near, z_far);

        // If the camera is parented under a TransformComponent, use that transform as the camera pose.
        // Otherwise default to identity view.
        let view = if let Some(model) = TransformSystem::world_model(world, component) {
            invert_affine_transform(&model)
        } else {
            Camera3D::identity().view
        };

        let cam = Camera3D { view, proj };

        let h = CameraHandle(self.next_handle);
        self.next_handle = self.next_handle.wrapping_add(1);

        self.cameras.push((h, AnyCamera::Camera3D(cam)));
        self.camera3d_components.insert(h, component);

        // Newest becomes active (window target).
        self.active_window_camera = Some(h);
        visuals.set_camera(cam.view, cam.proj);

        h
    }

    pub fn set_active_window_camera(&mut self, visuals: &mut VisualWorld, h: CameraHandle) {
        if self.active_window_camera == Some(h) {
            return;
        }

        if let Some((_, cam)) = self.cameras.iter().find(|(ch, _)| *ch == h) {
            self.active_window_camera = Some(h);
            match *cam {
                AnyCamera::Camera3D(cam3d) => {
                    visuals.set_camera(cam3d.view, cam3d.proj);
                }
                AnyCamera::Camera2D(cam2d) => {
                    visuals.set_camera(cam2d.view, cam2d.proj);
                }
            }
        }
    }

    pub fn set_active_xr_camera(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        // Only allow selecting an enabled XR camera; otherwise keep existing selection.
        if world
            .get_component_by_id_as::<CameraXRComponent>(component)
            .is_some_and(|c| c.enabled)
        {
            self.active_xr_camera = Some(component);
            visuals.set_active_xr_camera(Some(component));
        }
    }

    /// Update Camera2D view/proj from the component tree.
    ///
    /// `camera2d_component_id` should be the Camera2DComponent, whose parent is typically a TransformComponent.
    pub fn update_camera_2d_from_parent_transform(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        camera2d_component_id: ComponentId,
        transform_component_id: ComponentId,
    ) {
        let Some(camera2d_comp) = world
            .get_component_by_id_as::<crate::engine::ecs::component::Camera2DComponent>(
                camera2d_component_id,
            )
        else {
            return;
        };

        if let Some(handle) = camera2d_comp.handle {
            if self.active_window_camera == Some(handle) {
                // Maintain the old call sites' contract: ensure the provided parent really is a Transform.
                if world
                    .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                        transform_component_id,
                    )
                    .is_none()
                {
                    return;
                }

                // Use the full accumulated world model for the camera component so nested transforms work.
                let model = TransformSystem::world_model(world, camera2d_component_id)
                    .unwrap_or_else(|| Camera3D::identity().view);

                let view = invert_affine_transform(&model);

                // Match the previous shader-side aspect correction: scale X by (height/width).
                let vp = visuals.viewport();
                let inv_aspect = if vp[0] > 0.0 { vp[1] / vp[0] } else { 1.0 };
                let proj = [
                    [inv_aspect, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                // Persist into the camera registry so switching cameras updates visuals immediately.
                if let Some((_, AnyCamera::Camera2D(cam2d))) =
                    self.cameras.iter_mut().find(|(ch, _)| *ch == handle)
                {
                    cam2d.view = view;
                    cam2d.proj = proj;
                }

                visuals.set_camera(view, proj);
            }
        }
    }

    /// Register a Camera2D component.
    pub fn register_camera2d(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) -> CameraHandle {
        let h = CameraHandle(self.next_handle);
        self.next_handle = self.next_handle.wrapping_add(1);

        self.cameras
            .push((h, AnyCamera::Camera2D(Camera2D::identity())));
        self.camera2d_components.insert(h, component);

        // Newest becomes active (window target).
        self.active_window_camera = Some(h);

        h
    }

    pub fn active_window_camera_matrices(&self) -> Option<([[f32; 4]; 4], [[f32; 4]; 4])> {
        let h = self.active_window_camera?;
        let (_, cam) = self.cameras.iter().find(|(ch, _)| *ch == h)?;
        match *cam {
            AnyCamera::Camera3D(cam3d) => Some((cam3d.view, cam3d.proj)),
            AnyCamera::Camera2D(cam2d) => Some((cam2d.view, cam2d.proj)),
        }
    }

    /// Update Camera3D view/proj from the component tree.
    ///
    /// `camera3d_component_id` should be the Camera3DComponent, whose parent is typically a TransformComponent.
    pub fn update_camera_3d_from_parent_transform(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        camera3d_component_id: ComponentId,
        transform_component_id: ComponentId,
    ) {
        let Some(camera3d_comp) = world
            .get_component_by_id_as::<crate::engine::ecs::component::Camera3DComponent>(
                camera3d_component_id,
            )
        else {
            return;
        };

        if let Some(handle) = camera3d_comp.handle {
            if self.active_window_camera == Some(handle) {
                // Maintain the old call sites' contract: ensure the provided parent really is a Transform.
                if world
                    .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                        transform_component_id,
                    )
                    .is_none()
                {
                    return;
                }

                // Use the full accumulated world model for the camera component so nested transforms work.
                let model = TransformSystem::world_model(world, camera3d_component_id)
                    .unwrap_or_else(|| Camera3D::identity().view);
                let view = invert_affine_transform(&model);

                let vp = visuals.viewport();
                let aspect = if vp[1] > 0.0 { vp[0] / vp[1] } else { 1.0 };
                let proj = Camera3D::perspective_rh_zo(
                    camera3d_comp.fov_y_degrees.to_radians(),
                    aspect,
                    camera3d_comp.z_near,
                    camera3d_comp.z_far,
                );

                // Persist into the camera registry so switching cameras updates visuals immediately.
                if let Some((_, AnyCamera::Camera3D(cam3d))) =
                    self.cameras.iter_mut().find(|(ch, _)| *ch == handle)
                {
                    cam3d.view = view;
                    cam3d.proj = proj;
                }

                visuals.set_camera(view, proj);
            }
        }
    }

    fn update_active_xr_camera(&mut self, world: &World, visuals: &mut VisualWorld) {
        // If the current active XR camera is still enabled, keep it.
        if let Some(active) = self.active_xr_camera {
            if let Some(c) = world.get_component_by_id_as::<CameraXRComponent>(active) {
                if c.enabled {
                    visuals.set_active_xr_camera(Some(active));
                    return;
                }
            }
        }

        // Otherwise, pick the first enabled XR camera component (if any).
        let next = world.all_components().find(|&id| {
            world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some_and(|c| c.enabled)
        });

        self.active_xr_camera = next;
        visuals.set_active_xr_camera(next);
    }
}

/// Invert an affine 4x4 transform matrix (upper 3x3 + translation).
///
/// Assumes the bottom row is `[0, 0, 0, 1]` (which matches `Transform::recompute_model`).
/// Returns identity if the 3x3 part is singular.
fn invert_affine_transform(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Upper-left 3x3 in column-major.
    let c0 = [m[0][0], m[0][1], m[0][2]];
    let c1 = [m[1][0], m[1][1], m[1][2]];
    let c2 = [m[2][0], m[2][1], m[2][2]];

    // Row-major elements for determinant/cofactors.
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
        return Camera3D::identity().view;
    }
    let inv_det = 1.0 / det;

    // Inverse in row-major.
    let inv00 = (a11 * a22 - a12 * a21) * inv_det;
    let inv01 = (a02 * a21 - a01 * a22) * inv_det;
    let inv02 = (a01 * a12 - a02 * a11) * inv_det;

    let inv10 = (a12 * a20 - a10 * a22) * inv_det;
    let inv11 = (a00 * a22 - a02 * a20) * inv_det;
    let inv12 = (a02 * a10 - a00 * a12) * inv_det;

    let inv20 = (a10 * a21 - a11 * a20) * inv_det;
    let inv21 = (a01 * a20 - a00 * a21) * inv_det;
    let inv22 = (a00 * a11 - a01 * a10) * inv_det;

    // Translation.
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

impl System for CameraSystem {
    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &crate::engine::user_input::InputState,
        _dt_sec: f32,
    ) {
        // Maintain which XR rig is active so the OpenXR system can apply the correct world transform.
        self.update_active_xr_camera(world, visuals);

        let vp = visuals.viewport();
        let prev_vp = self.last_viewport;
        self.last_viewport = Some(vp);

        // If the viewport changed, projections that depend on aspect ratio may need updating.
        // View matrices are updated event-driven via TransformSystem::transform_changed.
        let viewport_changed = prev_vp != Some(vp);

        if !viewport_changed {
            return;
        }

        let Some(active_handle) = self.active_window_camera else {
            // No camera in the scene: keep the legacy behavior where 2D content is aspect-correct.
            // Previously this was done in the vertex shader via `ubo.viewport`.
            let inv_aspect = if vp[0] > 0.0 { vp[1] / vp[0] } else { 1.0 };
            let view = Camera3D::identity().view;
            let proj = [
                [inv_aspect, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            visuals.set_camera(view, proj);
            return;
        };

        // View is already up-to-date via TransformSystem; on resize we only need to refresh proj.
        let Some((_, cam)) = self.cameras.iter_mut().find(|(ch, _)| *ch == active_handle) else {
            return;
        };

        match cam {
            AnyCamera::Camera2D(cam2d) => {
                let inv_aspect = if vp[0] > 0.0 { vp[1] / vp[0] } else { 1.0 };
                cam2d.proj = [
                    [inv_aspect, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ];
                visuals.set_camera(cam2d.view, cam2d.proj);
            }
            AnyCamera::Camera3D(cam3d) => {
                let aspect = if vp[1] > 0.0 { vp[0] / vp[1] } else { 1.0 };

                let (fov_y_deg, z_near, z_far) = self
                    .camera3d_components
                    .get(&active_handle)
                    .and_then(|cid| world.get_component_by_id_as::<Camera3DComponent>(*cid))
                    .map(|c| (c.fov_y_degrees, c.z_near, c.z_far))
                    .unwrap_or((
                        Camera3DComponent::DEFAULT_FOV_Y_DEGREES,
                        Camera3DComponent::DEFAULT_Z_NEAR,
                        Camera3DComponent::DEFAULT_Z_FAR,
                    ));

                cam3d.proj =
                    Camera3D::perspective_rh_zo(fov_y_deg.to_radians(), aspect, z_near, z_far);
                visuals.set_camera(cam3d.view, cam3d.proj);
            }
        }
    }
}
