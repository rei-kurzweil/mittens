//! `App` namespace: ask the embedding application to switch scenes.
use super::runtime_config::{MittensApi, MittensBinding};
use crate::engine::app_launch::{self, LaunchRequest};
use meow_meow_script as mms;

pub(super) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    builder.namespace("App", |namespace| {
        namespace.api(
            "launch",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Null),
            MittensBinding::Api(MittensApi::AppLaunch),
        );
        namespace.api(
            "home",
            mms::ValueSignature::new(Vec::new(), mms::ValueType::Null),
            MittensBinding::Api(MittensApi::AppHome),
        );
        namespace.api(
            "last_error",
            mms::ValueSignature::new(Vec::new(), mms::ValueType::String),
            MittensBinding::Api(MittensApi::AppLastError),
        );
    });
}

pub(super) fn supports(api: MittensApi) -> bool {
    matches!(
        api,
        MittensApi::AppLaunch | MittensApi::AppHome | MittensApi::AppLastError
    )
}

pub(super) fn invoke(
    api: MittensApi,
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    use mms::TransportValue as V;
    let (operation, request) = match (api, args.as_slice()) {
        (MittensApi::AppLaunch, [V::String(path)]) => {
            ("App.launch", LaunchRequest::Scene(path.clone()))
        }
        (MittensApi::AppLaunch, _) => {
            return Err(mms::HostError::failure(
                "App.launch",
                "expected one string path argument",
            ));
        }
        (MittensApi::AppHome, []) => ("App.home", LaunchRequest::Home),
        (MittensApi::AppLastError, []) => {
            return Ok(mms::HostResponse::Transport(V::String(
                app_launch::last_error(),
            )));
        }
        (api, _) => {
            return Err(mms::HostError::failure(
                format!("{api:?}"),
                "expects no arguments",
            ));
        }
    };
    app_launch::request(request)
        .map_err(|error| mms::HostError::unavailable(operation, error))?;
    Ok(mms::HostResponse::Unit)
}
