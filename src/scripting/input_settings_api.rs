use super::MittensHost;
use crate::engine::ecs::component::XrStickMode;
use crate::engine::ecs::system::input_settings_system::settings_file;
use crate::scripting::runtime_config::{MittensApi, MittensBinding};
use crate::scripting::xr_settings_api::file_option;
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::{ValueSignature as Sig, ValueType as T};
    builder.namespace("InputSettings", |namespace| {
        namespace.namespace("xr", |xr| {
            xr.api(
                "stick_mode",
                Sig::with_optional(vec![T::Table], 0, T::String),
                MittensBinding::Api(MittensApi::InputXrStickModeGet),
            );
            xr.api(
                "set_stick_mode",
                Sig::with_optional(vec![T::String, T::Table], 1, T::Null),
                MittensBinding::Api(MittensApi::InputXrStickModeSet),
            );
            xr.api(
                "default_file",
                Sig::new(vec![], T::String),
                MittensBinding::Api(MittensApi::InputXrSettingsDefaultFile),
            );
        });
    });
}

impl MittensHost<'_, '_> {
    /// With `{file}`, reads the saved mode (falling back to the current one
    /// when nothing is saved) or saves before applying.
    pub(super) fn invoke_input_settings(
        &mut self,
        api: MittensApi,
        args: &[mms::TransportValue],
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as S, TransportValue as V};
        let operation = match api {
            MittensApi::InputXrStickModeGet => "InputSettings.xr.stick_mode",
            MittensApi::InputXrStickModeSet => "InputSettings.xr.set_stick_mode",
            _ => "InputSettings.xr.default_file",
        };
        let fail = |e: String| mms::HostError::failure(operation, e);
        if api == MittensApi::InputXrSettingsDefaultFile {
            if !args.is_empty() {
                return Err(fail("expects no arguments".into()));
            }
            let path = settings_file::default_file().map_err(fail)?;
            let path = path
                .to_str()
                .ok_or_else(|| fail("config path is not UTF-8".into()))?;
            return Ok(S::Transport(V::String(path.into())));
        }
        let settings = self.input_settings.as_deref_mut().ok_or_else(|| {
            mms::HostError::unavailable(operation, "input settings are unavailable")
        })?;
        if api == MittensApi::InputXrStickModeGet {
            let file = file_option(args).map_err(|e| fail(e.into()))?;
            let mode = match file {
                Some(file) => settings_file::load(file).map_err(fail)?,
                None => None,
            }
            .unwrap_or_else(|| settings.xr_stick_mode());
            return Ok(S::Transport(V::String(mode.as_str().into())));
        }
        let Some((V::String(mode), options)) = args.split_first() else {
            return Err(fail("expects a stick mode string and optional file options".into()));
        };
        let mode = XrStickMode::parse(mode)
            .ok_or_else(|| fail("expected \"strafe\" or \"turn\"".into()))?;
        match file_option(options).map_err(|e| fail(e.into()))? {
            Some(file) => settings
                .save_xr_stick_mode(file, mode, self.emit)
                .map_err(fail)?,
            None => settings.set_xr_stick_mode(mode, self.emit),
        }
        Ok(S::Unit)
    }
}
