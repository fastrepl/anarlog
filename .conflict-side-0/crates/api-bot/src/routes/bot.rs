use std::sync::Arc;

use axum::{Extension, Json, extract::Path, response::Html};
use hypr_recall::{
    AutomaticLeaveConfig, BotStatusCode, BotVariant, CreateBotRequest, EveryoneLeftConfig,
    OutputMedia, OutputMediaConfig, OutputMediaKind, OutputMediaWebpageConfig,
    RealTimeTranscriptionConfig, RecallClient, RecordingConfig, SilenceDetectionConfig,
    StartRecordingOn, TranscriptionOptions, TranscriptionProvider, VariantKind,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::BotConfig;
use crate::error::Result;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendBotRequest {
    pub meeting_url: String,
    #[serde(default)]
    pub bot_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SendBotResponse {
    pub bot_id: String,
}

#[utoipa::path(
    post,
    path = "/bot",
    request_body = SendBotRequest,
    responses(
        (status = 200, description = "Bot sent to meeting", body = SendBotResponse),
        (status = 400, description = "Bad request"),
        (status = 502, description = "Recall API error"),
    )
)]
pub async fn send_bot(
    Extension(config): Extension<Arc<BotConfig>>,
    Extension(client): Extension<Arc<RecallClient>>,
    Json(req): Json<SendBotRequest>,
) -> Result<Json<SendBotResponse>> {
    let transcript_url = format!("{}/bot/webhook/transcript", config.public_url);

    let bot = client
        .create_bot(CreateBotRequest {
            meeting_url: req.meeting_url,
            bot_name: req.bot_name.unwrap_or_else(|| "Hyprnote".into()),
            transcription_options: Some(TranscriptionOptions {
                provider: TranscriptionProvider::MeetingCaptions,
            }),
            real_time_transcription: Some(RealTimeTranscriptionConfig {
                destination_url: transcript_url,
                partial_results: false,
            }),
            output_media: None,
            metadata: None,
            automatic_leave: None,
            recording_config: None,
            variant: None,
        })
        .await?;

    Ok(Json(SendBotResponse { bot_id: bot.id }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartDemoRequest {
    pub meeting_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StartDemoResponse {
    pub bot_id: String,
    pub meeting_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DemoStatusResponse {
    pub status: String,
    pub ready: bool,
}

#[utoipa::path(
    post,
    path = "/onboarding/demo",
    request_body = StartDemoRequest,
    responses(
        (status = 200, description = "Onboarding bot dispatched", body = StartDemoResponse),
        (status = 502, description = "Recall API error"),
    )
)]
pub async fn start_demo(
    Extension(config): Extension<Arc<BotConfig>>,
    Extension(client): Extension<Arc<RecallClient>>,
    Json(req): Json<StartDemoRequest>,
) -> Result<Json<StartDemoResponse>> {
    let player_url = format!("{}/onboarding/player", config.public_url);

    let bot = client
        .create_bot(CreateBotRequest {
            meeting_url: req.meeting_url.clone(),
            bot_name: "Hyprnote Demo".into(),
            transcription_options: None,
            real_time_transcription: None,
            output_media: Some(OutputMedia {
                camera: Some(OutputMediaConfig {
                    kind: OutputMediaKind::Webpage,
                    config: OutputMediaWebpageConfig { url: player_url },
                }),
                screenshare: None,
            }),
            metadata: None,
            // Disable automatic recording — this bot only plays video, recording is useless and costly.
            recording_config: Some(RecordingConfig {
                start_recording_on: Some(StartRecordingOn::Manual),
            }),
            // Use web_4_core for smooth video playback (9x the default CPU).
            variant: Some(BotVariant {
                zoom: Some(VariantKind::Web4Core),
                google_meet: Some(VariantKind::Web4Core),
                microsoft_teams: Some(VariantKind::Web4Core),
                webex: None,
            }),
            automatic_leave: Some(AutomaticLeaveConfig {
                // Leave promptly after video ends — the page goes silent once playback finishes.
                silence_detection: Some(SilenceDetectionConfig {
                    timeout: Some(10),
                    activate_after: Some(1),
                }),
                noone_joined_timeout: Some(120),
                everyone_left_timeout: Some(EveryoneLeftConfig {
                    timeout: Some(5),
                    activate_after: None,
                }),
                ..Default::default()
            }),
        })
        .await?;

    Ok(Json(StartDemoResponse {
        bot_id: bot.id,
        meeting_url: req.meeting_url,
    }))
}

#[utoipa::path(
    get,
    path = "/onboarding/demo/{bot_id}",
    params(("bot_id" = String, Path, description = "Recall bot ID")),
    responses(
        (status = 200, description = "Bot status", body = DemoStatusResponse),
        (status = 502, description = "Recall API error"),
    )
)]
pub async fn demo_status(
    Extension(client): Extension<Arc<RecallClient>>,
    Path(bot_id): Path<String>,
) -> Result<Json<DemoStatusResponse>> {
    let bot = client.get_bot(&bot_id).await?;
    let ready = matches!(
        bot.status.code,
        BotStatusCode::InCallNotRecording | BotStatusCode::InCallRecording
    );

    Ok(Json(DemoStatusResponse {
        status: format!("{:?}", bot.status.code),
        ready,
    }))
}

#[utoipa::path(
    delete,
    path = "/bot/{bot_id}",
    params(("bot_id" = String, Path, description = "Recall bot ID")),
    responses(
        (status = 200, description = "Bot removed from call"),
        (status = 502, description = "Recall API error"),
    )
)]
pub async fn remove_bot(
    Extension(client): Extension<Arc<RecallClient>>,
    Path(bot_id): Path<String>,
) -> Result<()> {
    client.remove_bot(&bot_id).await?;

    Ok(())
}

pub async fn player(Extension(config): Extension<Arc<BotConfig>>) -> Html<String> {
    let url = html_escape(&config.demo_video_url);
    Html(format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; background: #000; }}
    #done {{
      display: none;
      color: #fff;
      font-family: sans-serif;
      font-size: 28px;
      justify-content: center;
      align-items: center;
      width: 1280px;
      height: 720px;
    }}
  </style>
</head>
<body>
  <video
    id="v"
    src="{url}"
    autoplay
    playsinline
    preload="auto"
    style="width:1280px;height:720px;display:block"
  ></video>
  <div id="done">Thanks for watching!</div>
  <script>
    var v = document.getElementById('v');
    var done = document.getElementById('done');
    function showDone() {{
      v.style.display = 'none';
      done.style.display = 'flex';
    }}
    v.addEventListener('ended', showDone);
    v.addEventListener('error', showDone);
  </script>
</body>
</html>"#
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
