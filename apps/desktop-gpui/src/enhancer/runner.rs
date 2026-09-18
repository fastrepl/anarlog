//! `tasks.ts`'s `generate` loop over one workflow: the stream with its start
//! and idle timeouts, `withEarlyValidationRetry`'s buffered retries, the
//! bullet normaliser, the reasoning step, and the output cap. Runs on the
//! tokio runtime and reports progress over a channel.

use std::time::Duration;

use tokio::sync::mpsc;

use super::text::BulletNormalizer;
use super::validator::{EarlyCheck, EnhanceValidator, early_check};
use crate::llm_stream::{self, Chunk, Connection, Request};

pub const TASK_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(15);
pub const TASK_STREAM_START_TIMEOUT: Duration = Duration::from_secs(60);
/// On-device models can spend minutes loading weights before the first token.
pub const TASK_STREAM_LOCAL_START_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const MAX_AI_TASK_STREAM_CHARACTERS: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Generating,
    Reasoning,
    Retrying { attempt: usize, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Step(Step),
    /// The full text so far.
    Text(String),
    Done(String),
    Error(String),
}

pub struct Generation {
    pub conn: Connection,
    pub system: String,
    pub prompt: String,
    /// `args.imageContext`: the note's images for a model that takes them.
    pub images: Vec<llm_stream::ImagePart>,
    pub max_output_tokens: u32,
    /// `withEarlyValidationRetry` when set; the title task streams straight.
    pub validator: Option<EnhanceValidator>,
    /// `normalizeBulletPoints` (the enhance workflow's transform).
    pub normalize_bullets: bool,
}

/// Start the generation; progress arrives until `Done` or `Error`. Dropping
/// the receiver aborts the run.
pub fn run(
    runtime: &tokio::runtime::Handle,
    generation: Generation,
) -> mpsc::UnboundedReceiver<Progress> {
    let (sender, receiver) = mpsc::unbounded_channel();
    let handle = runtime.clone();
    runtime.spawn(async move {
        let result = run_inner(&handle, generation, &sender).await;
        match result {
            Ok(text) => {
                let _ = sender.send(Progress::Done(text));
            }
            Err(message) => {
                let _ = sender.send(Progress::Error(message));
            }
        }
    });
    receiver
}

fn start_timeout(conn: &Connection) -> Duration {
    if llm_stream::is_local_model_provider(&conn.provider_id) {
        TASK_STREAM_LOCAL_START_TIMEOUT
    } else {
        TASK_STREAM_START_TIMEOUT
    }
}

async fn run_inner(
    runtime: &tokio::runtime::Handle,
    generation: Generation,
    sender: &mpsc::UnboundedSender<Progress>,
) -> Result<String, String> {
    let _ = sender.send(Progress::Step(Step::Generating));
    let max_retries = if generation.validator.is_some() {
        super::validator::EARLY_MAX_RETRIES
    } else {
        1
    };
    let mut previous_feedback: Option<String> = None;
    let mut full_text = String::new();

    for attempt in 0..max_retries {
        let prompt = match &previous_feedback {
            Some(feedback) => super::prompts::with_retry_feedback(&generation.prompt, feedback),
            None => generation.prompt.clone(),
        };
        let mut stream = llm_stream::stream(
            runtime,
            generation.conn.clone(),
            Request::with_images(
                generation.system.clone(),
                prompt,
                generation.images.clone(),
                generation.max_output_tokens,
            ),
        );
        let mut normalizer = BulletNormalizer::default();
        // The early-validation buffer for this attempt.
        let mut buffer = String::new();
        let mut validation_complete = generation.validator.is_none();
        let mut retry_feedback: Option<String> = None;
        full_text.clear();
        let mut reasoning_active = false;

        loop {
            let timeout = if full_text.trim().is_empty() {
                start_timeout(&generation.conn)
            } else {
                TASK_STREAM_IDLE_TIMEOUT
            };
            let next = match tokio::time::timeout(timeout, stream.recv()).await {
                Ok(next) => next,
                Err(_) => {
                    // `STREAM_TIMEOUT`: keep what streamed, or fail with no text.
                    if full_text.trim().is_empty() && buffer.trim().is_empty() {
                        return Err("AI generation did not return any text.".to_string());
                    }
                    if !buffer.is_empty() {
                        full_text.push_str(&buffer);
                        let _ = sender.send(Progress::Text(full_text.clone()));
                    }
                    return Ok(full_text);
                }
            };
            let Some(chunk) = next else {
                break;
            };
            match chunk {
                Chunk::Error(message) => return Err(message),
                Chunk::Done => break,
                // The enhancer advertises no tools and keeps no item ids.
                Chunk::ToolCall(_) | Chunk::Item(_) => {}
                Chunk::ReasoningDelta(_) => {
                    if !reasoning_active && full_text.is_empty() && buffer.is_empty() {
                        reasoning_active = true;
                        let _ = sender.send(Progress::Step(Step::Reasoning));
                    }
                }
                Chunk::TextDelta(text) => {
                    let text = if generation.normalize_bullets {
                        normalizer.push(&text)
                    } else {
                        text
                    };
                    if reasoning_active {
                        reasoning_active = false;
                        let _ = sender.send(Progress::Step(Step::Generating));
                    }
                    if !validation_complete {
                        buffer.push_str(&text);
                        let validator = generation.validator.as_ref().expect("validator set");
                        let (check, feedback) = early_check(validator, &buffer, attempt);
                        match check {
                            EarlyCheck::Buffer => continue,
                            EarlyCheck::Retry => {
                                retry_feedback = feedback;
                                break;
                            }
                            EarlyCheck::Flush | EarlyCheck::GiveUp => {
                                // `onGiveUp` / `onRetrySuccess` both show `generating`.
                                if check == EarlyCheck::GiveUp || attempt > 0 {
                                    let _ = sender.send(Progress::Step(Step::Generating));
                                }
                                validation_complete = true;
                                let flushed = std::mem::take(&mut buffer);
                                append(&mut full_text, &flushed, sender)?;
                            }
                        }
                    } else {
                        append(&mut full_text, &text, sender)?;
                    }
                }
            }
        }

        if let Some(feedback) = retry_feedback {
            drop(stream);
            let _ = sender.send(Progress::Step(Step::Retrying {
                attempt: attempt + 1,
                reason: feedback.clone(),
            }));
            previous_feedback = Some(feedback);
            continue;
        }
        if !buffer.is_empty() {
            append(&mut full_text, &buffer, sender)?;
        }
        return Ok(full_text);
    }
    Ok(full_text)
}

fn append(
    full_text: &mut String,
    text: &str,
    sender: &mpsc::UnboundedSender<Progress>,
) -> Result<(), String> {
    if full_text.len() + text.len() > MAX_AI_TASK_STREAM_CHARACTERS {
        return Err("AI generation exceeded the safe output limit.".to_string());
    }
    full_text.push_str(text);
    let _ = sender.send(Progress::Text(full_text.clone()));
    Ok(())
}
