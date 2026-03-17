use crate::cli::{ConnectProvider, ConnectionType};

pub(crate) enum Effect {
    Save {
        connection_type: ConnectionType,
        provider: ConnectProvider,
        base_url: Option<String>,
        api_key: Option<String>,
    },
    Exit,
}
