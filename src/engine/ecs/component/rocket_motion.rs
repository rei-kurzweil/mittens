//! Native motion providers and opt-in projectile impact observation.
use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy)]
pub(crate) struct DriveWheel {
    pub steering: ComponentId,
    pub spin: ComponentId,
    pub radius: f32,
    pub front: bool,
    pub steer_angle: f32,
    pub roll_angle: f32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DriveAcceleration {
    pub throttle: f32,
    pub steering: f32,
    pub braking: bool,
    pub wheels: [Option<DriveWheel>; 4],
}

#[derive(Debug, Clone)]
pub struct AccelerationComponent {
    pub local: [f32; 3],
    pub max_speed: f32,
    pub(crate) drive: Option<DriveAcceleration>,
}
impl Default for AccelerationComponent {
    fn default() -> Self {
        Self {
            local: [0.0; 3],
            max_speed: f32::MAX,
            drive: None,
        }
    }
}
impl AccelerationComponent {
    pub(crate) fn configure_drive(&mut self) {
        self.max_speed = 5.0;
        self.drive = Some(DriveAcceleration::default());
    }

    pub(crate) fn set_drive(
        &mut self,
        throttle: f32,
        steering: f32,
        braking: bool,
    ) -> Result<(), String> {
        if !(-1.0..=1.0).contains(&throttle) || !(-1.0..=1.0).contains(&steering) {
            return Err("Acceleration.set_drive expects throttle and steering in [-1, 1]".into());
        }
        let drive = self
            .drive
            .as_mut()
            .ok_or("set_drive requires Acceleration.drive()")?;
        drive.throttle = throttle;
        drive.steering = steering;
        drive.braking = braking;
        Ok(())
    }

    pub(crate) fn bind_drive_wheel(
        &mut self,
        steering: ComponentId,
        spin: ComponentId,
        radius: f32,
        front: bool,
    ) -> Result<(), String> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err("Acceleration.bind_wheel expects a positive wheel radius".into());
        }
        let drive = self
            .drive
            .as_mut()
            .ok_or("bind_wheel requires Acceleration.drive()")?;
        let slot = drive
            .wheels
            .iter_mut()
            .find(|slot| slot.is_none_or(|wheel| wheel.spin == spin))
            .ok_or("Acceleration.drive supports four wheel bindings")?;
        *slot = Some(DriveWheel {
            steering,
            spin,
            radius,
            front,
            steer_angle: 0.0,
            roll_angle: 0.0,
        });
        Ok(())
    }
}
#[derive(Debug, Clone, Default)]
pub struct ProjectileImpactComponent;
impl Component for AccelerationComponent {
    fn name(&self) -> &'static str {
        "acceleration"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use super::ce_helpers::*;
        if self.drive.is_some() {
            return ce_call("Acceleration", "drive", vec![])
                .with_call("max_speed", vec![num(self.max_speed as f64)]);
        }
        ce_call(
            "Acceleration",
            "local",
            vec![crate::scripting::ast::Expression::Array(nums(
                self.local.into_iter().map(f64::from),
            ))],
        )
        .with_call("max_speed", vec![num(self.max_speed as f64)])
    }
}
impl Component for ProjectileImpactComponent {
    fn name(&self) -> &'static str {
        "projectile_impact"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(
        &self,
        _: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        super::ce_helpers::ce("ProjectileImpact")
    }
}
