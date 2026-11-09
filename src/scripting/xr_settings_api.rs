//! XR settings API registration and explicit calibration file operations.
use super::MittensHost;
use crate::engine::ecs::system::xr_settings_system::calibration_file;
use crate::scripting::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::{ValueSignature as Sig, ValueType as T};
    builder.namespace("XrSettings", |namespace| {
        namespace.namespace("floor", |floor| {
            floor.api(
                "set_correction_meters",
                Sig::with_optional(vec![T::F64, T::Table], 1, T::Null),
                MittensBinding::Api(MittensApi::XrFloorCorrectionSet),
            );
            floor.api(
                "correction_meters",
                Sig::with_optional(vec![T::Table], 0, T::F64),
                MittensBinding::Api(MittensApi::XrFloorCorrectionGet),
            );
            floor.api(
                "default_file",
                Sig::new(vec![], T::String),
                MittensBinding::Api(MittensApi::XrFloorDefaultFile),
            );
        });
    });
}

pub(crate) fn supports(api: MittensApi) -> bool {
    matches!(
        api,
        MittensApi::XrFloorCorrectionSet
            | MittensApi::XrFloorCorrectionGet
            | MittensApi::XrFloorDefaultFile
    )
}

fn file_option(args: &[mms::TransportValue]) -> Result<Option<&str>, &'static str> {
    use mms::TransportValue as V;
    match args {
        [] => Ok(None),
        [V::Table(fields)] => match fields.as_slice() {
            [(key, V::String(file))] if key == "file" => Ok(Some(file)),
            _ => Err("options must contain exactly one string field: file"),
        },
        _ => Err("expected an optional {file = filename} table"),
    }
}

impl MittensHost<'_, '_> {
    pub(super) fn invoke_xr_settings(
        &mut self,
        api: MittensApi,
        args: Vec<mms::TransportValue>,
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as S, TransportValue as V};
        let operation = match api {
            MittensApi::XrFloorCorrectionSet => "XrSettings.floor.set_correction_meters",
            MittensApi::XrFloorCorrectionGet => "XrSettings.floor.correction_meters",
            MittensApi::XrFloorDefaultFile => "XrSettings.floor.default_file",
            _ => unreachable!(),
        };
        let fail = |e| mms::HostError::failure(operation, e);
        if api == MittensApi::XrFloorDefaultFile {
            if !args.is_empty() {
                return Err(fail("expects no arguments".to_string()));
            }
            let path = calibration_file::default_file().map_err(fail)?;
            let path = path
                .to_str()
                .ok_or_else(|| fail("config path is not UTF-8".into()))?;
            return Ok(S::Transport(V::String(path.into())));
        }
        let settings = self.xr_settings.as_deref_mut().ok_or_else(|| {
            mms::HostError::unavailable(operation, "XR settings service is unavailable")
        })?;
        if api == MittensApi::XrFloorCorrectionGet {
            let file = file_option(&args).map_err(|e| fail(e.into()))?;
            let meters = match file {
                Some(file) => calibration_file::load(file).map_err(fail)?,
                None => settings.floor_correction_meters() as f64,
            };
            return Ok(S::Transport(V::Number(meters)));
        }
        let Some((V::Number(meters), options)) = args.split_first() else {
            return Err(fail("expects a number and optional file options".into()));
        };
        let file = file_option(options).map_err(|e| fail(e.into()))?;
        if let Some(file) = file {
            settings
                .save_floor_correction_meters(file, *meters, self.emit)
                .map_err(fail)?;
        } else {
            settings
                .set_floor_correction_meters(*meters, self.emit)
                .map_err(|e| fail(e.into()))?;
        }
        Ok(S::Unit)
    }
}
