use super::MittensHost;
use crate::engine::ecs::component::XrStickMode;
use crate::scripting::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::{ValueSignature as Sig, ValueType as T};
    builder.namespace("InputSettings", |namespace| {
        namespace.namespace("xr", |xr| {
            xr.api(
                "stick_mode",
                Sig::new(vec![], T::String),
                MittensBinding::Api(MittensApi::InputXrStickModeGet),
            );
            xr.api(
                "set_stick_mode",
                Sig::new(vec![T::String], T::Null),
                MittensBinding::Api(MittensApi::InputXrStickModeSet),
            );
        });
    });
}

impl MittensHost<'_, '_> {
    pub(super) fn invoke_input_settings(
        &mut self,
        args: &[mms::TransportValue],
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as S, TransportValue as V};
        let settings = self.input_settings.as_deref_mut().ok_or_else(|| {
            mms::HostError::unavailable("InputSettings.xr", "input settings are unavailable")
        })?;
        match args {
            [] => Ok(S::Transport(V::String(
                settings.xr_stick_mode().as_str().into(),
            ))),
            [V::String(mode)] => {
                let mode = XrStickMode::parse(mode).ok_or_else(|| {
                    mms::HostError::failure(
                        "InputSettings.xr.set_stick_mode",
                        "expected \"strafe\" or \"turn\"",
                    )
                })?;
                settings.set_xr_stick_mode(mode, self.emit);
                Ok(S::Unit)
            }
            _ => Err(mms::HostError::failure(
                "InputSettings.xr",
                "expected a stick mode string",
            )),
        }
    }
}
