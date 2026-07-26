use ractor::concurrency::Duration;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tauri_specta::Event;

use crate::event::NetworkStatusEvent;

const CHECK_INTERVAL: Duration = Duration::from_secs(2);
const CHECK_URL: &str = "https://www.google.com/generate_204";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub const NETWORK_ACTOR_NAME: &str = "network_actor";

pub enum NetworkMsg {
    Check,
}

pub struct NetworkArgs {
    pub app: tauri::AppHandle,
}

pub struct NetworkState {
    app: tauri::AppHandle,
    is_online: bool,
}

pub struct NetworkActor;

impl NetworkActor {
    pub fn name() -> ractor::ActorName {
        NETWORK_ACTOR_NAME.into()
    }
}

#[ractor::async_trait]
impl Actor for NetworkActor {
    type Msg = NetworkMsg;
    type State = NetworkState;
    type Arguments = NetworkArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        schedule_check(myself);

        Ok(NetworkState {
            app: args.app,
            is_online: true,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            NetworkMsg::Check => {
                let is_online = check_network().await;

                if is_online != state.is_online {
                    state.is_online = is_online;

                    let event = NetworkStatusEvent { is_online };
                    if let Err(e) = event.emit(&state.app) {
                        tracing::error!(?e, "failed_to_emit_network_status_event");
                    }
                }

                schedule_check(myself);
            }
        }
        Ok(())
    }
}

fn schedule_check(actor: ActorRef<NetworkMsg>) {
    ractor::time::send_after(CHECK_INTERVAL, actor.get_cell(), || NetworkMsg::Check);
}

async fn check_network() -> bool {
    let client = reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client.head(CHECK_URL).send().await {
        Ok(response) => response.status().is_success() || response.status().as_u16() == 204,
        Err(_) => false,
    }
}
