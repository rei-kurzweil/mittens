use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use crate::engine::ecs::component::ce_helpers::{ce, ce_call, num};
use crate::engine::graphics::mesh::MeshFactory;
use crate::engine::graphics::primitives::{
    CpuMeshHandle, InstanceHandle, MaterialHandle, Renderable,
};
use crate::engine::graphics::render_assets::RenderAssets;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthoredRenderableShape {
    Builtin(&'static str),
    Cone {
        segments: u32,
    },
    Icosahedron {
        tessellations: u32,
        sphericalness: f32,
    },
    PartialAnnulus2d {
        inner_radius: f32,
        outer_radius: f32,
        start_angle_radians: f32,
        sweep_angle_radians: f32,
        segments: u32,
    },
    Star {
        points: u32,
        inner_radius_fraction: f32,
        outer_bevel_segments: u32,
        inner_bevel_segments: u32,
    },
    Heart {
        segments: u32,
    },
    WireframeBox {
        thickness: f32,
    },
    WireframeSquare {
        thickness: f32,
    },
    WireframeSphere {
        latitude_segments: u32,
        longitude_segments: u32,
        thickness: f32,
    },
    WireframeIcosahedron {
        tessellations: u32,
        sphericalness: f32,
        thickness: f32,
    },
}

/// Renderable component.
#[derive(Debug, Clone)]
pub struct RenderableComponent {
    pub renderable: Renderable,
    pub authored_shape: Option<AuthoredRenderableShape>,

    /// VisualWorld instance handle created for this renderable.
    pub handle: Option<InstanceHandle>,

    component: Option<ComponentId>,
}

impl RenderableComponent {
    pub fn new(renderable: Renderable) -> Self {
        Self {
            renderable,
            authored_shape: None,
            handle: None,
            component: None,
        }
    }

    pub fn from_cpu_mesh_handle(h: CpuMeshHandle, material: MaterialHandle) -> Self {
        Self::new(Renderable::new(h, material))
    }

    pub fn get_handle(&self) -> Option<InstanceHandle> {
        self.handle
    }

    /// Predefined renderable: 2D triangle (shared built-in mesh handle).
    pub fn triangle() -> Self {
        let mut s =
            Self::from_cpu_mesh_handle(CpuMeshHandle::TRIANGLE_2D, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("triangle"));
        s
    }

    /// Predefined renderable: 2D triangle (unique CPU mesh registered into `render_assets`).
    pub fn triangle_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::triangle_2d());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH)
                .with_base_mesh(CpuMeshHandle::TRIANGLE_2D),
        )
    }

    /// Predefined renderable: 2D square/quad (shared built-in mesh handle).
    pub fn square() -> Self {
        let mut s = Self::from_cpu_mesh_handle(CpuMeshHandle::QUAD_2D, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("square"));
        s
    }

    /// Predefined renderable: 2D plane/quad (alias of `square`).
    pub fn plane() -> Self {
        Self::square()
    }

    /// Predefined renderable: 2D square/quad (unique CPU mesh registered into `render_assets`).
    pub fn square_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::quad_2d());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::QUAD_2D),
        )
    }

    /// Predefined renderable: cube primitive (shared built-in mesh handle).
    pub fn cube() -> Self {
        let mut s = Self::from_cpu_mesh_handle(CpuMeshHandle::CUBE, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("cube"));
        s
    }

    /// Predefined renderable: cube primitive (unique CPU mesh registered into `render_assets`).
    pub fn cube_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::cube());
        Self::new(Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::CUBE))
    }

    /// Unit wireframe box with twelve solid edges of configurable relative thickness.
    pub fn wireframe_box(render_assets: &mut RenderAssets, thickness: f32) -> Self {
        let thickness = thickness.clamp(1.0e-4, 1.0);
        let handle = render_assets.wireframe_box_mesh(thickness);
        let mut component = Self::new(
            Renderable::new(handle, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::CUBE),
        );
        component.authored_shape = Some(AuthoredRenderableShape::WireframeBox { thickness });
        component
    }

    /// Unit wireframe square in the XY plane with configurable relative edge thickness.
    pub fn wireframe_square(render_assets: &mut RenderAssets, thickness: f32) -> Self {
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let handle = render_assets.wireframe_square_mesh(thickness);
        let mut component = Self::new(
            Renderable::new(handle, MaterialHandle::TOON_MESH)
                .with_base_mesh(CpuMeshHandle::QUAD_2D),
        );
        component.authored_shape = Some(AuthoredRenderableShape::WireframeSquare { thickness });
        component
    }

    pub fn wireframe_sphere(
        render_assets: &mut RenderAssets,
        latitude_segments: u32,
        longitude_segments: u32,
        thickness: f32,
    ) -> Self {
        let latitude_segments = latitude_segments.max(2);
        let longitude_segments = longitude_segments.max(3);
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let handle =
            render_assets.wireframe_sphere_mesh(latitude_segments, longitude_segments, thickness);
        let mut component = Self::new(Renderable::new(handle, MaterialHandle::TOON_MESH));
        component.authored_shape = Some(AuthoredRenderableShape::WireframeSphere {
            latitude_segments,
            longitude_segments,
            thickness,
        });
        component
    }

    pub fn wireframe_icosahedron(
        render_assets: &mut RenderAssets,
        tessellations: u32,
        sphericalness: f32,
        thickness: f32,
    ) -> Self {
        let sphericalness = sphericalness.clamp(0.0, 1.0);
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let handle =
            render_assets.wireframe_icosahedron_mesh(tessellations, sphericalness, thickness);
        let mut component = Self::new(Renderable::new(handle, MaterialHandle::TOON_MESH));
        component.authored_shape = Some(AuthoredRenderableShape::WireframeIcosahedron {
            tessellations,
            sphericalness,
            thickness,
        });
        component
    }

    /// Predefined renderable: sphere primitive (shared built-in mesh handle).
    pub fn sphere() -> Self {
        let mut s = Self::from_cpu_mesh_handle(CpuMeshHandle::SPHERE, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("sphere"));
        s
    }

    /// Predefined renderable: sphere primitive (unique CPU mesh registered into `render_assets`).
    pub fn sphere_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::sphere());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::SPHERE),
        )
    }

    /// Predefined renderable: cone primitive (shared built-in mesh handle).
    pub fn cone() -> Self {
        let mut s = Self::from_cpu_mesh_handle(CpuMeshHandle::CONE, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("cone"));
        s
    }

    /// Predefined renderable: cone primitive (unique CPU mesh registered into `render_assets`).
    pub fn cone_dynamic(render_assets: &mut RenderAssets, segments: u32) -> Self {
        let h = render_assets.register_mesh(MeshFactory::cone(segments));
        let mut s = Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH).with_base_mesh(CpuMeshHandle::CONE),
        );
        s.authored_shape = Some(AuthoredRenderableShape::Cone { segments });
        s
    }

    pub fn icosahedron(
        render_assets: &mut RenderAssets,
        tessellations: u32,
        sphericalness: f32,
    ) -> Self {
        let h = render_assets.register_mesh(MeshFactory::icosahedron(tessellations, sphericalness));
        let mut s = Self::new(Renderable::new(h, MaterialHandle::TOON_MESH));
        s.authored_shape = Some(AuthoredRenderableShape::Icosahedron {
            tessellations,
            sphericalness,
        });
        s
    }

    /// Predefined renderable: tetrahedron primitive (shared built-in mesh handle).
    pub fn tetrahedron() -> Self {
        let mut s =
            Self::from_cpu_mesh_handle(CpuMeshHandle::TETRAHEDRON, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("tetrahedron"));
        s
    }

    /// Predefined renderable: tetrahedron primitive (unique CPU mesh registered into `render_assets`).
    pub fn tetrahedron_dynamic(render_assets: &mut RenderAssets) -> Self {
        let h = render_assets.register_mesh(MeshFactory::tetrahedron());
        Self::new(
            Renderable::new(h, MaterialHandle::TOON_MESH)
                .with_base_mesh(CpuMeshHandle::TETRAHEDRON),
        )
    }

    /// Predefined renderable: tetrahedron (alias of `tetrahedron`).
    pub fn color_tetrahedron() -> Self {
        Self::tetrahedron()
    }

    /// Predefined renderable: 2D circle (shared built-in mesh handle).
    pub fn circle2d() -> Self {
        let mut s = Self::from_cpu_mesh_handle(CpuMeshHandle::CIRCLE_2D, MaterialHandle::TOON_MESH);
        s.authored_shape = Some(AuthoredRenderableShape::Builtin("circle2d"));
        s
    }

    pub fn partial_annulus_2d(
        render_assets: &mut RenderAssets,
        inner_radius: f32,
        outer_radius: f32,
        start_angle_radians: f32,
        sweep_angle_radians: f32,
        segments: u32,
    ) -> Self {
        let mesh = MeshFactory::partial_annulus_2d(
            inner_radius,
            outer_radius,
            start_angle_radians,
            sweep_angle_radians,
            segments,
        );
        let h = render_assets.register_mesh(mesh);
        let mut s = Self::new(Renderable::new(h, MaterialHandle::TOON_MESH));
        s.authored_shape = Some(AuthoredRenderableShape::PartialAnnulus2d {
            inner_radius,
            outer_radius,
            start_angle_radians,
            sweep_angle_radians,
            segments,
        });
        s
    }

    pub fn star(
        render_assets: &mut RenderAssets,
        points: u32,
        inner_radius_fraction: f32,
        outer_bevel_segments: u32,
        inner_bevel_segments: u32,
    ) -> Self {
        let h = render_assets.register_mesh(MeshFactory::star(
            points,
            inner_radius_fraction,
            outer_bevel_segments,
            inner_bevel_segments,
        ));
        let mut s = Self::new(Renderable::new(h, MaterialHandle::TOON_MESH));
        s.authored_shape = Some(AuthoredRenderableShape::Star {
            points,
            inner_radius_fraction,
            outer_bevel_segments,
            inner_bevel_segments,
        });
        s
    }

    pub fn heart(render_assets: &mut RenderAssets, segments: u32) -> Self {
        let h = render_assets.register_mesh(MeshFactory::heart(segments));
        let mut s = Self::new(Renderable::new(h, MaterialHandle::TOON_MESH));
        s.authored_shape = Some(AuthoredRenderableShape::Heart { segments });
        s
    }
}

impl Component for RenderableComponent {
    fn name(&self) -> &'static str {
        "renderable"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterRenderable {
                component_ids: vec![component],
            },
        );
    }

    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveRenderable {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        match &self.authored_shape {
            Some(AuthoredRenderableShape::Builtin(name)) => ce_call("Renderable", name, vec![]),
            Some(AuthoredRenderableShape::Cone { segments }) => {
                ce_call("Renderable", "cone", vec![num(*segments as f64)])
            }
            Some(AuthoredRenderableShape::Icosahedron {
                tessellations,
                sphericalness,
            }) => ce_call(
                "Renderable",
                "icosahedron",
                vec![num(*tessellations as f64), num(*sphericalness as f64)],
            ),
            Some(AuthoredRenderableShape::PartialAnnulus2d {
                inner_radius,
                outer_radius,
                start_angle_radians,
                sweep_angle_radians,
                segments,
            }) => ce_call(
                "Renderable",
                "partial_annulus_2d",
                vec![
                    num(*inner_radius as f64),
                    num(*outer_radius as f64),
                    num(*start_angle_radians as f64),
                    num(*sweep_angle_radians as f64),
                    num(*segments as f64),
                ],
            ),
            Some(AuthoredRenderableShape::Star {
                points,
                inner_radius_fraction,
                outer_bevel_segments,
                inner_bevel_segments,
            }) => ce_call(
                "Renderable",
                "star",
                vec![
                    num(*points as f64),
                    num(*inner_radius_fraction as f64),
                    num(*outer_bevel_segments as f64),
                    num(*inner_bevel_segments as f64),
                ],
            ),
            Some(AuthoredRenderableShape::Heart { segments }) => {
                ce_call("Renderable", "heart", vec![num(*segments as f64)])
            }
            Some(AuthoredRenderableShape::WireframeBox { thickness }) => {
                ce_call("Renderable", "wireframe_box", vec![num(*thickness as f64)])
            }
            Some(AuthoredRenderableShape::WireframeSquare { thickness }) => ce_call(
                "Renderable",
                "wireframe_square",
                vec![num(*thickness as f64)],
            ),
            Some(AuthoredRenderableShape::WireframeSphere {
                latitude_segments,
                longitude_segments,
                thickness,
            }) => ce_call(
                "Renderable",
                "wireframe_sphere",
                vec![
                    num(*latitude_segments as f64),
                    num(*longitude_segments as f64),
                    num(*thickness as f64),
                ],
            ),
            Some(AuthoredRenderableShape::WireframeIcosahedron {
                tessellations,
                sphericalness,
                thickness,
            }) => ce_call(
                "Renderable",
                "wireframe_icosahedron",
                vec![
                    num(*tessellations as f64),
                    num(*sphericalness as f64),
                    num(*thickness as f64),
                ],
            ),
            None => ce("Renderable"),
        }
    }
}
