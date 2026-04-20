use napi::bindgen_prelude::{Function, block_on, spawn};
use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi_derive::napi;

use crate::error::to_napi_error;
use crate::state::require_state_blocking;
use crate::subscription::SubscriptionHandle;

#[napi(object)]
pub struct LiveQueryDelta {
    pub event: String,
    pub rows: Option<Vec<serde_json::Value>>,
    pub error: Option<String>,
    pub reactive: bool,
}

#[napi]
pub fn subscribe(
    sql: String,
    params: Vec<serde_json::Value>,
    callback: Function<'_, LiveQueryDelta, ()>,
) -> napi::Result<SubscriptionHandle> {
    let state = require_state_blocking()?;
    let mut watch =
        block_on(hypr_api::live::subscribe(&state, sql, params)).map_err(to_napi_error)?;
    let reactive = watch.reactive();
    let tsfn = callback
        .build_threadsafe_function::<LiveQueryDelta>()
        .callee_handled::<false>()
        .build()
        .map_err(to_napi_error)?;
    let (cancel, mut cancel_rx) = tokio::sync::oneshot::channel();

    spawn(async move {
        loop {
            tokio::select! {
                _ = &mut cancel_rx => break,
                next = watch.next() => {
                    let Some(next) = next else { break };

                    let delta = match next {
                        Ok(data) => LiveQueryDelta {
                            event: "snapshot".to_string(),
                            rows: Some(data.rows),
                            error: None,
                            reactive: data.reactive,
                        },
                        Err(error) => LiveQueryDelta {
                            event: "error".to_string(),
                            rows: None,
                            error: Some(error),
                            reactive: watch.reactive(),
                        },
                    };

                    if tsfn.call(delta, ThreadsafeFunctionCallMode::NonBlocking)
                        != napi::Status::Ok
                    {
                        break;
                    }
                }
            }
        }

        let _ = watch.close().await;
    });

    Ok(SubscriptionHandle::new(cancel, reactive))
}
