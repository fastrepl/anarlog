use rig::message::Message;
use tokio::sync::mpsc;

use crate::agent::Backend;
use crate::error::{CliError, CliResult};
use crate::llm::ResolvedLlmConfig;

pub(crate) enum RuntimeEvent {
    Chunk(String),
    Completed(Option<String>),
    Failed(String),
    TitleGenerated(String),
}

pub(crate) struct Runtime {
    backend: Backend,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    max_turns: usize,
}

impl Runtime {
    pub(crate) fn new(
        config: ResolvedLlmConfig,
        system_message: Option<String>,
        tx: mpsc::UnboundedSender<RuntimeEvent>,
    ) -> CliResult<Self> {
        Ok(Self {
            backend: Backend::new(config, system_message)?,
            tx,
            max_turns: 1,
        })
    }

    pub(crate) fn generate_title(&self, prompt: String, response: String) {
        let backend = self.backend.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let title_prompt = format!(
                "Generate a short title (3-5 words) for this conversation. Reply with ONLY the title, no quotes or punctuation.\n\nUser: {prompt}\nAssistant: {response}"
            );
            let result = backend
                .stream_text(title_prompt, Vec::new(), 1, |_| Ok(()))
                .await;
            if let Ok(Some(title)) = result {
                let title = title.trim().to_string();
                if !title.is_empty() {
                    let _ = tx.send(RuntimeEvent::TitleGenerated(title));
                }
            }
        });
    }

    pub(crate) fn submit(&self, prompt: String, history: Vec<Message>) {
        let backend = self.backend.clone();
        let tx = self.tx.clone();
        let max_turns = self.max_turns;

        tokio::spawn(async move {
            let final_text = match backend
                .stream_text(prompt, history, max_turns, |chunk| {
                    tx.send(RuntimeEvent::Chunk(chunk.to_string()))
                        .map_err(|e| CliError::operation_failed("chat stream", e.to_string()))?;
                    Ok(())
                })
                .await
            {
                Ok(final_text) => final_text,
                Err(error) => {
                    let _ = tx.send(RuntimeEvent::Failed(error.to_string()));
                    return;
                }
            };

            let _ = tx.send(RuntimeEvent::Completed(final_text));
        });
    }
}
