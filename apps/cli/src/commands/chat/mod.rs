mod action;
mod app;
mod effect;
mod runtime;
mod ui;

use std::time::Duration;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Resume an existing chat session
    Resume {
        #[arg(long)]
        meeting: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum Role {
    System,
    User,
    Assistant,
    Tool,
}

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};
use crate::llm::{LlmProvider, resolve_config};

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: Duration = Duration::from_secs(1);

pub struct Args {
    pub meeting: Option<String>,
    pub prompt: Option<String>,
    pub provider: Option<LlmProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub pool: SqlitePool,
    pub resume_meeting_id: Option<String>,
}

struct ChatScreen {
    app: App,
    runtime: Runtime,
}

impl ChatScreen {
    fn new(app: App, runtime: Runtime) -> Self {
        Self { app, runtime }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Submit { prompt, history } => {
                    self.runtime.submit(prompt, history);
                }
                Effect::GenerateTitle { prompt, response } => {
                    self.runtime.generate_title(prompt, response);
                }
                Effect::Persist {
                    meeting_id,
                    message_id,
                    role,
                    content,
                } => {
                    self.runtime
                        .persist_message(meeting_id, message_id, role, content);
                }
                Effect::UpdateTitle { meeting_id, title } => {
                    self.runtime.update_title(meeting_id, title);
                }
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for ChatScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = ();

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                let effects = self.app.dispatch(Action::Paste(pasted));
                self.apply_effects(effects)
            }
            TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            RuntimeEvent::Chunk(chunk) => Action::StreamChunk(chunk),
            RuntimeEvent::ToolCallStarted {
                tool_name,
                arguments,
            } => Action::ToolCallStarted {
                tool_name,
                arguments,
            },
            RuntimeEvent::Completed(final_text) => Action::StreamCompleted(final_text),
            RuntimeEvent::Failed(error) => Action::StreamFailed(error),
            RuntimeEvent::TitleGenerated(title) => Action::TitleGenerated(title),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn next_frame_delay(&self) -> Duration {
        IDLE_FRAME
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let pool = args.pool;
    let system_message = match args.meeting.as_deref() {
        Some(meeting_id) => Some(load_meeting_context(&pool, meeting_id).await?),
        None => None,
    };
    let config = resolve_config(
        &pool,
        args.provider,
        args.base_url,
        args.api_key,
        args.model,
    )
    .await?;

    if let Some(prompt) = args.prompt {
        return crate::agent::run_prompt(config, system_message, &prompt).await;
    }

    let meeting_id = args
        .resume_meeting_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let (runtime_tx, runtime_rx) = mpsc::unbounded_channel();
    let runtime = Runtime::new(config.clone(), system_message, runtime_tx, pool.clone())?;

    let mut app = App::new(config.model, args.meeting, meeting_id.clone());

    let history = load_or_create_meeting(&pool, &meeting_id).await?;
    if let Some(messages) = history {
        app.load_history(messages);
    } else {
        runtime.ensure_meeting(&meeting_id).await;
    }

    let runtime_handle = runtime.clone();
    let result = run_screen(ChatScreen::new(app, runtime), Some(runtime_rx))
        .await
        .map_err(|e| CliError::operation_failed("chat tui", e.to_string()));

    runtime_handle.drain_pending_writes().await;
    result
}

async fn load_meeting_context(pool: &SqlitePool, meeting_id: &str) -> CliResult<String> {
    let meeting = hypr_db_app::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("get meeting", e.to_string()))?
        .ok_or_else(|| CliError::not_found(format!("meeting '{meeting_id}'"), None))?;

    let words = hypr_db_app::load_words(pool, meeting_id)
        .await
        .unwrap_or_default();
    let transcript_text = {
        let mut sorted = words;
        sorted.sort_by_key(|w| w.start_ms);
        let text: String = sorted
            .iter()
            .map(|w| w.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if text.is_empty() { None } else { Some(text) }
    };

    let participant_rows = hypr_db_app::list_meeting_participants(pool, meeting_id)
        .await
        .unwrap_or_default();
    let mut participants = Vec::new();
    for row in &participant_rows {
        if let Ok(Some(human)) = hypr_db_app::get_human(pool, &row.human_id).await {
            if !human.name.is_empty() {
                participants.push(hypr_template_cli::Participant {
                    name: human.name,
                    job_title: if human.job_title.is_empty() {
                        None
                    } else {
                        Some(human.job_title)
                    },
                });
            }
        }
    }

    let memo = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "memo")
        .await
        .ok()
        .flatten()
        .map(|n| n.content)
        .filter(|v| !v.trim().is_empty());
    let summary = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "summary")
        .await
        .ok()
        .flatten()
        .map(|n| n.content)
        .filter(|v| !v.trim().is_empty());

    if meeting.title.is_none() && memo.is_none() && summary.is_none() && transcript_text.is_none() {
        return Err(CliError::operation_failed(
            "load meeting context",
            format!("meeting '{meeting_id}' has no transcript, memo, or summary"),
        ));
    }

    let ctx = hypr_template_cli::ChatContext {
        meeting_id: meeting_id.to_string(),
        title: meeting.title,
        created_at: Some(meeting.created_at),
        participants,
        memo,
        summary,
        transcript_text,
    };

    ctx.render()
        .map_err(|e| CliError::operation_failed("render meeting context", e.to_string()))
}

async fn load_or_create_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> CliResult<Option<Vec<hypr_db_app::ChatMessageRow>>> {
    let meeting = hypr_db_app::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("get meeting", e.to_string()))?;
    match meeting {
        Some(_) => {
            let messages = hypr_db_app::load_chat_messages(pool, meeting_id)
                .await
                .unwrap_or_default();
            Ok(Some(messages))
        }
        None => Ok(None),
    }
}
