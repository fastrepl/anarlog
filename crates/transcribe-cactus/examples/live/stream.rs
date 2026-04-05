use std::time::{Duration, Instant};

use futures_util::StreamExt;
use owhisper_client::FinalizeHandle;
use owhisper_interface::stream::StreamResponse;

use crate::TIMEOUT_SECS;
use crate::display::{ChannelKind, DisplayMode, Transcript};

pub async fn process<S, H>(response_stream: S, handle: H, mode: DisplayMode)
where
    S: futures_util::Stream<Item = Result<StreamResponse, owhisper_client::hypr_ws_client::Error>>,
    H: FinalizeHandle,
{
    futures_util::pin_mut!(response_stream);

    let t0 = Instant::now();
    let mut channels: Vec<(Transcript, Option<String>)> = match &mode {
        DisplayMode::Single(kind) => vec![(Transcript::new(t0, *kind), None)],
        DisplayMode::Dual => vec![
            (Transcript::new(t0, ChannelKind::Mic), None),
            (Transcript::new(t0, ChannelKind::Speaker), None),
        ],
    };

    let read_loop = async {
        while let Some(result) = response_stream.next().await {
            match result {
                Ok(StreamResponse::TranscriptResponse {
                    is_final,
                    channel,
                    channel_index,
                    ..
                }) => {
                    let text = channel
                        .alternatives
                        .first()
                        .map(|a| a.transcript.as_str())
                        .unwrap_or("");

                    let ch = match &mode {
                        DisplayMode::Single(_) => 0,
                        DisplayMode::Dual => {
                            channel_index.first().copied().unwrap_or(0).clamp(0, 1) as usize
                        }
                    };

                    let (transcript, last_confirmed) = &mut channels[ch];
                    if is_final {
                        if last_confirmed.as_deref() == Some(text) {
                            continue;
                        }
                        *last_confirmed = Some(text.to_string());
                        transcript.confirm(text);
                    } else {
                        transcript.set_partial(text);
                    }
                }
                Ok(StreamResponse::TerminalResponse { .. }) => break,
                Ok(StreamResponse::ErrorResponse { error_message, .. }) => {
                    eprintln!("\nerror: {}", error_message);
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("\nws error: {:?}", e);
                    break;
                }
            }
        }
    };

    let _ = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), read_loop).await;
    handle.finalize().await;
    eprintln!();
}
