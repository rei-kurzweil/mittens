//! Built-in graphics settings table and host dispatch, shared by all MMS panels.
#[cfg(test)]
#[path = "graphics_settings_panel_tests.rs"]
mod tests;
use super::MittensHost;
use crate::scripting::{
    DeferredCallbackHostPhase,
    runtime_config::{MittensApi, MittensBinding},
};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    builder.namespace("GraphicsSettings", |namespace| {
        namespace.namespace("foveation", |foveation| {
            foveation.api(
                "quality",
                mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::FoveationQualitySet),
            );
            foveation.api(
                "quality_level",
                mms::ValueSignature::new(vec![], mms::ValueType::String),
                MittensBinding::Api(MittensApi::FoveationQualityGet),
            );
        });
        namespace.namespace("bloom", |bloom| {
            bloom.api(
                "toggle",
                mms::ValueSignature::new(vec![mms::ValueType::Bool], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::BloomToggle),
            );
            bloom.api(
                "enabled",
                mms::ValueSignature::new(vec![], mms::ValueType::Bool),
                MittensBinding::Api(MittensApi::BloomEnabled),
            );
            bloom.api(
                "layers",
                mms::ValueSignature::new(vec![mms::ValueType::I32], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::BloomLayers),
            );
            bloom.api(
                "layer_count",
                mms::ValueSignature::new(vec![], mms::ValueType::I32),
                MittensBinding::Api(MittensApi::BloomLayerCount),
            );
        });
        namespace.namespace("mirrors", |mirrors| {
            mirrors.api(
                "toggle",
                mms::ValueSignature::new(vec![mms::ValueType::Bool], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::MirrorsToggle),
            );
            mirrors.api(
                "enabled",
                mms::ValueSignature::new(vec![], mms::ValueType::Bool),
                MittensBinding::Api(MittensApi::MirrorsEnabled),
            );
            mirrors.api(
                "quality",
                mms::ValueSignature::new(vec![mms::ValueType::I32], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::MirrorsQuality),
            );
            mirrors.api(
                "quality_override",
                mms::ValueSignature::new(vec![], mms::ValueType::Any),
                MittensBinding::Api(MittensApi::MirrorsQualityOverride),
            );
            mirrors.api(
                "reset_quality",
                mms::ValueSignature::new(vec![], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::MirrorsResetQuality),
            );
        });
        namespace.namespace("secondary_motion", |motion| {
            motion.api(
                "toggle",
                mms::ValueSignature::new(vec![mms::ValueType::Bool], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::SecondaryMotionToggle),
            );
            motion.api(
                "enabled",
                mms::ValueSignature::new(vec![], mms::ValueType::Bool),
                MittensBinding::Api(MittensApi::SecondaryMotionEnabled),
            );
            motion.api(
                "pause",
                mms::ValueSignature::new(vec![mms::ValueType::Bool], mms::ValueType::Null),
                MittensBinding::Api(MittensApi::SecondaryMotionPause),
            );
            motion.api(
                "paused",
                mms::ValueSignature::new(vec![], mms::ValueType::Bool),
                MittensBinding::Api(MittensApi::SecondaryMotionPaused),
            );
        });
    });
}

pub(crate) fn supports(api: MittensApi) -> bool {
    matches!(
        api,
        MittensApi::MirrorsToggle
            | MittensApi::MirrorsEnabled
            | MittensApi::MirrorsQuality
            | MittensApi::MirrorsQualityOverride
            | MittensApi::MirrorsResetQuality
            | MittensApi::BloomToggle
            | MittensApi::BloomEnabled
            | MittensApi::BloomLayers
            | MittensApi::BloomLayerCount
            | MittensApi::SecondaryMotionToggle
            | MittensApi::SecondaryMotionEnabled
            | MittensApi::SecondaryMotionPause
            | MittensApi::SecondaryMotionPaused
            | MittensApi::FoveationQualitySet
            | MittensApi::FoveationQualityGet
    )
}

impl MittensHost<'_, '_> {
    pub(super) fn invoke_graphics_settings(
        &mut self,
        api: MittensApi,
        args: Vec<mms::TransportValue>,
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::HostResponse as S;
        match &MittensBinding::Api(api) {
            MittensBinding::Api(MittensApi::MirrorsToggle) => {
                let [mms::TransportValue::Bool(enabled)] = args.as_slice() else {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.mirrors.toggle",
                        "expects one boolean",
                    ));
                };
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.mirrors.toggle",
                        "graphics settings service is unavailable",
                    )
                })?;
                if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                    settings.set_mirrors_enabled(*enabled, self.emit);
                }
                Ok(S::Unit)
            }
            MittensBinding::Api(MittensApi::MirrorsEnabled) => {
                if !args.is_empty() {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.mirrors.enabled",
                        "expects no arguments",
                    ));
                }
                let settings = self.graphics_settings.as_deref().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.mirrors.enabled",
                        "graphics settings service is unavailable",
                    )
                })?;
                Ok(S::Transport(mms::TransportValue::Bool(
                    settings.mirrors_enabled(),
                )))
            }
            MittensBinding::Api(MittensApi::MirrorsQuality) => {
                let [mms::TransportValue::Number(quality)] = args.as_slice() else {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.mirrors.quality",
                        "expects one i32",
                    ));
                };
                if !quality.is_finite()
                    || quality.fract() != 0.0
                    || *quality < i32::MIN as f64
                    || *quality > i32::MAX as f64
                {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.mirrors.quality",
                        "expects one i32",
                    ));
                }
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.mirrors.quality",
                        "graphics settings service is unavailable",
                    )
                })?;
                if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                    settings.set_mirror_quality(*quality as i32, self.emit);
                }
                Ok(S::Unit)
            }
            MittensBinding::Api(
                api @ (MittensApi::MirrorsQualityOverride | MittensApi::MirrorsResetQuality),
            ) => {
                let reset = *api == MittensApi::MirrorsResetQuality;
                let operation = if reset {
                    "GraphicsSettings.mirrors.reset_quality"
                } else {
                    "GraphicsSettings.mirrors.quality_override"
                };
                if !args.is_empty() {
                    return Err(mms::HostError::failure(operation, "expects no arguments"));
                }
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        operation,
                        "graphics settings service is unavailable",
                    )
                })?;
                if reset {
                    if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                        settings.reset_mirror_quality(self.emit);
                    }
                    Ok(S::Unit)
                } else {
                    Ok(S::Transport(
                        settings
                            .mirror_quality_override()
                            .map(|q| mms::TransportValue::Number(q as f64))
                            .unwrap_or(mms::TransportValue::Null),
                    ))
                }
            }
            MittensBinding::Api(
                api @ (MittensApi::BloomToggle
                | MittensApi::BloomEnabled
                | MittensApi::BloomLayers
                | MittensApi::BloomLayerCount),
            ) => {
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.bloom",
                        "graphics settings service is unavailable",
                    )
                })?;
                match api {
                    MittensApi::BloomEnabled => Ok(S::Transport(mms::TransportValue::Bool(
                        settings.bloom_enabled(),
                    ))),
                    MittensApi::BloomLayerCount => Ok(S::Transport(mms::TransportValue::Number(
                        settings.bloom_layers() as f64,
                    ))),
                    MittensApi::BloomToggle => {
                        let [mms::TransportValue::Bool(value)] = args.as_slice() else {
                            return Err(mms::HostError::failure(
                                "GraphicsSettings.bloom.toggle",
                                "expects one boolean",
                            ));
                        };
                        if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                            settings.set_bloom_enabled(*value, self.emit);
                        }
                        Ok(S::Unit)
                    }
                    MittensApi::BloomLayers => {
                        let [mms::TransportValue::Number(value)] = args.as_slice() else {
                            return Err(mms::HostError::failure(
                                "GraphicsSettings.bloom.layers",
                                "expects an integer in [1,6]",
                            ));
                        };
                        if !value.is_finite()
                            || value.fract() != 0.0
                            || !(1.0..=6.0).contains(value)
                        {
                            return Err(mms::HostError::failure(
                                "GraphicsSettings.bloom.layers",
                                "expects an integer in [1,6]",
                            ));
                        }
                        if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                            settings
                                .set_bloom_layers(*value as u32, self.emit)
                                .map_err(|e| {
                                    mms::HostError::failure("GraphicsSettings.bloom.layers", e)
                                })?;
                        }
                        Ok(S::Unit)
                    }
                    _ => unreachable!(),
                }
            }
            MittensBinding::Api(MittensApi::SecondaryMotionToggle) => {
                let [mms::TransportValue::Bool(enabled)] = args.as_slice() else {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.secondary_motion.toggle",
                        "expects one boolean",
                    ));
                };
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.secondary_motion.toggle",
                        "graphics settings service is unavailable",
                    )
                })?;
                if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                    settings.set_secondary_motion_enabled(*enabled, self.emit);
                }
                Ok(S::Unit)
            }
            MittensBinding::Api(MittensApi::SecondaryMotionEnabled) => {
                if !args.is_empty() {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.secondary_motion.enabled",
                        "expects no arguments",
                    ));
                }
                let settings = self.graphics_settings.as_deref().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.secondary_motion.enabled",
                        "graphics settings service is unavailable",
                    )
                })?;
                Ok(S::Transport(mms::TransportValue::Bool(
                    settings.secondary_motion_enabled(),
                )))
            }
            MittensBinding::Api(MittensApi::SecondaryMotionPause) => {
                let [mms::TransportValue::Bool(paused)] = args.as_slice() else {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.secondary_motion.pause",
                        "expects one boolean",
                    ));
                };
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.secondary_motion.pause",
                        "graphics settings service is unavailable",
                    )
                })?;
                if self.deferred_callback_phase != Some(DeferredCallbackHostPhase::Audio) {
                    settings.set_secondary_motion_paused(*paused, self.emit);
                }
                Ok(S::Unit)
            }
            MittensBinding::Api(MittensApi::SecondaryMotionPaused) => {
                if !args.is_empty() {
                    return Err(mms::HostError::failure(
                        "GraphicsSettings.secondary_motion.paused",
                        "expects no arguments",
                    ));
                }
                let settings = self.graphics_settings.as_deref().ok_or_else(|| {
                    mms::HostError::unavailable(
                        "GraphicsSettings.secondary_motion.paused",
                        "graphics settings service is unavailable",
                    )
                })?;
                Ok(S::Transport(mms::TransportValue::Bool(
                    settings.secondary_motion_paused(),
                )))
            }
            MittensBinding::Api(MittensApi::FoveationQualitySet) => {
                let operation = "GraphicsSettings.foveation.quality";
                let [mms::TransportValue::String(value)] = args.as_slice() else {
                    return Err(mms::HostError::failure(
                        operation,
                        "expects off, low, medium or high",
                    ));
                };
                let quality = crate::engine::ecs::system::graphics_settings_system::XrFoveationQuality::parse(value)
                    .ok_or_else(|| mms::HostError::failure(operation, "expects off, low, medium or high"))?;
                let settings = self.graphics_settings.as_deref_mut().ok_or_else(|| {
                    mms::HostError::unavailable(
                        operation,
                        "graphics settings service is unavailable",
                    )
                })?;
                settings.set_foveation_quality(quality, self.emit);
                Ok(S::Unit)
            }
            MittensBinding::Api(MittensApi::FoveationQualityGet) => {
                let operation = "GraphicsSettings.foveation.quality_level";
                if !args.is_empty() {
                    return Err(mms::HostError::failure(operation, "expects no arguments"));
                }
                let settings = self.graphics_settings.as_deref().ok_or_else(|| {
                    mms::HostError::unavailable(
                        operation,
                        "graphics settings service is unavailable",
                    )
                })?;
                Ok(S::Transport(mms::TransportValue::String(
                    settings.foveation_quality().as_str().into(),
                )))
            }
            _ => Err(mms::HostError::failure(
                "GraphicsSettings",
                "unsupported graphics operation",
            )),
        }
    }
}
