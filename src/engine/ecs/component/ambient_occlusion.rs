use crate::engine::ecs::World;
use crate::engine::ecs::component::Component;
use crate::engine::graphics::ao_bake::{AoBakeSettings, AoDebug};

/// Which UV set carries the baked atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AoUvChannel {
    /// Replace `uv0`; the AO image becomes the surface's only texture.
    Texcoord0,
    /// A separate `uv1` set alongside the existing texture. Not implemented
    /// yet; see `docs/task/ambient-occlusion-baking.md` slice 7.
    Texcoord1,
}

/// Bakes ambient occlusion for the `CombineMesh` output it is attached to.
///
/// The combined mesh draws unshaded until the bake finishes, then switches
/// to an atlas-UV copy textured with the AO image. Identical batches share
/// one bake.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientOcclusionComponent {
    pub uv: AoUvChannel,
    pub settings: AoBakeSettings,
}

impl Default for AmbientOcclusionComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl AmbientOcclusionComponent {
    pub fn new() -> Self {
        Self {
            uv: AoUvChannel::Texcoord1,
            settings: AoBakeSettings::default(),
        }
    }

    pub fn texcoord0() -> Self {
        Self::new().with_texcoord0()
    }

    pub fn with_texcoord0(mut self) -> Self {
        self.uv = AoUvChannel::Texcoord0;
        self
    }

    pub fn with_resolution(mut self, texels: u32) -> Self {
        self.settings.resolution = texels;
        self
    }

    pub fn with_strength(mut self, strength: f32) -> Self {
        self.settings.strength = strength;
        self
    }

    pub fn with_proximity_radius(mut self, radius: f32) -> Self {
        self.settings.proximity_radius = radius;
        self
    }

    pub fn with_cavity(mut self, angle_deg: f32, radius: f32) -> Self {
        self.settings.cavity_angle_deg = angle_deg;
        self.settings.cavity_radius = radius;
        self
    }

    pub fn with_one_sided(mut self) -> Self {
        self.settings.two_sided = false;
        self
    }

    pub fn with_debug(mut self, debug: AoDebug) -> Self {
        self.settings.debug = debug;
        self
    }

    /// MMS builder calls. Numeric builders take one argument.
    pub fn apply_builder(&mut self, method: &str, value: Option<f32>) -> Result<(), String> {
        let number = |name: &str| {
            value
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("AmbientOcclusion.{name} expects one finite number"))
        };
        let non_negative = |name: &str| {
            let v = number(name)?;
            if v >= 0.0 {
                Ok(v)
            } else {
                Err(format!("AmbientOcclusion.{name} expects a value >= 0; got {v}"))
            }
        };
        match method {
            "texcoord0" => self.uv = AoUvChannel::Texcoord0,
            "one_sided" => self.settings.two_sided = false,
            "debug_checker" => self.settings.debug = AoDebug::Checker,
            "debug_proximity" => self.settings.debug = AoDebug::Proximity,
            "debug_cavity" => self.settings.debug = AoDebug::Cavity,
            "resolution" => {
                let v = number(method)?;
                if v.fract() != 0.0 || !(16.0..=8192.0).contains(&v) {
                    return Err(format!(
                        "AmbientOcclusion.resolution expects an integer in 16..=8192; got {v}"
                    ));
                }
                self.settings.resolution = v as u32;
            }
            "strength" => {
                let v = number(method)?;
                if !(0.0..=1.0).contains(&v) {
                    return Err(format!(
                        "AmbientOcclusion.strength expects a value in 0..=1; got {v}"
                    ));
                }
                self.settings.strength = v;
            }
            "proximity_radius" => self.settings.proximity_radius = non_negative(method)?,
            "cavity_radius" => self.settings.cavity_radius = non_negative(method)?,
            "cavity_angle" => {
                let v = number(method)?;
                if !(0.0..=180.0).contains(&v) {
                    return Err(format!(
                        "AmbientOcclusion.cavity_angle expects degrees in 0..=180; got {v}"
                    ));
                }
                self.settings.cavity_angle_deg = v;
            }
            _ => return Err(format!("AmbientOcclusion: unknown builder '{method}'")),
        }
        Ok(())
    }
}

impl Component for AmbientOcclusionComponent {
    fn name(&self) -> &'static str {
        "ambient_occlusion"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(&self, _world: &World) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::{CeBuilder, ce, num};
        let defaults = AoBakeSettings::default();
        let s = self.settings;
        let mut node = ce("AmbientOcclusion");
        if self.uv == AoUvChannel::Texcoord0 {
            node = node.with_call("texcoord0", vec![]);
        }
        if !s.two_sided {
            node = node.with_call("one_sided", vec![]);
        }
        for (name, value, default) in [
            ("resolution", s.resolution as f32, defaults.resolution as f32),
            ("strength", s.strength, defaults.strength),
            ("proximity_radius", s.proximity_radius, defaults.proximity_radius),
            ("cavity_radius", s.cavity_radius, defaults.cavity_radius),
            ("cavity_angle", s.cavity_angle_deg, defaults.cavity_angle_deg),
        ] {
            if value != default {
                node = node.with_call(name, vec![num(value as f64)]);
            }
        }
        match s.debug {
            AoDebug::Off => {}
            AoDebug::Checker => node = node.with_call("debug_checker", vec![]),
            AoDebug::Proximity => node = node.with_call("debug_proximity", vec![]),
            AoDebug::Cavity => node = node.with_call("debug_cavity", vec![]),
        }
        node
    }
}
