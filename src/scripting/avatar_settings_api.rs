use super::MittensHost;
use crate::scripting::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::{ValueSignature as Sig, ValueType as T};
    builder.namespace("AvatarSettings", |namespace| {
        namespace.namespace("height", |height| {
            height.api(
                "meters",
                Sig::new(vec![], T::Any),
                MittensBinding::Api(MittensApi::AvatarHeightGet),
            );
            height.api(
                "set_meters",
                Sig::new(vec![T::F64], T::Null),
                MittensBinding::Api(MittensApi::AvatarHeightSet),
            );
        });
    });
}

impl MittensHost<'_, '_> {
    pub(super) fn invoke_avatar_settings(
        &mut self,
        args: &[mms::TransportValue],
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as S, TransportValue as V};
        let settings = self.avatar_settings.as_deref_mut().ok_or_else(|| {
            mms::HostError::unavailable("AvatarSettings.height", "avatar settings are unavailable")
        })?;
        match args {
            [] => Ok(S::Transport(
                settings
                    .desired_height_meters()
                    .map(|height| V::Number(f64::from(height)))
                    .unwrap_or(V::Null),
            )),
            [V::Number(height)] => {
                settings
                    .set_desired_height_meters(*height, self.emit)
                    .map_err(|error| {
                        mms::HostError::failure("AvatarSettings.height.set_meters", error)
                    })?;
                Ok(S::Unit)
            }
            _ => Err(mms::HostError::failure(
                "AvatarSettings.height",
                "expected a height in meters",
            )),
        }
    }
}
