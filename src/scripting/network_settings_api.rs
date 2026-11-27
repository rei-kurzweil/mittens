use crate::engine::{
    ecs::{ComponentId, EventSignal},
    network_settings,
};
use crate::scripting::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(crate) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::{ValueSignature as Sig, ValueType as T};
    builder.namespace("NetworkSettings", |namespace| {
        namespace.namespace("asset_transfers", |transfers| {
            transfers.api(
                "auto_accept",
                Sig::new(vec![], T::Bool),
                MittensBinding::Api(MittensApi::NetworkAutoAcceptGet),
            );
            transfers.api(
                "set_auto_accept",
                Sig::new(vec![T::Bool], T::Null),
                MittensBinding::Api(MittensApi::NetworkAutoAcceptSet),
            );
        });
    });
}

impl super::MittensHost<'_, '_> {
    pub(super) fn invoke_network_settings(
        &mut self,
        args: &[mms::TransportValue],
    ) -> Result<mms::HostResponse, mms::HostError> {
        use mms::{HostResponse as R, TransportValue as V};
        let failure = |error| mms::HostError::failure("NetworkSettings.asset_transfers", error);
        match args {
            [] => Ok(R::Transport(V::Bool(
                network_settings::auto_accept_asset_transfers(),
            ))),
            [V::Bool(enabled)] => {
                network_settings::set_auto_accept_asset_transfers(*enabled).map_err(failure)?;
                self.emit.push_event(
                    ComponentId::default(),
                    EventSignal::DataEvent {
                        name: "NetworkSettingsChanged".into(),
                        payload: None,
                    },
                );
                Ok(R::Unit)
            }
            _ => Err(failure("expected one boolean for set_auto_accept".into())),
        }
    }
}
