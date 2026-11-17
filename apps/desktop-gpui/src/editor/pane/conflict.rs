//! The open note's conflict banner and version history act on the memo through these.

use std::sync::Arc;

use desktop_runtime::{DocumentSnapshot, ServiceError};
use gpui::{Context, Task};

use super::{EditorPane, with_title};
use crate::editor::{document::Document, model::EditorModel};

impl EditorPane {
    /// A save that lost to another revision of the note, once that revision has loaded.
    pub fn save_conflict(&self) -> bool {
        self.save_failed && self.external.is_some()
    }

    /// The conflict banner's "Keep this version": saves the draft over the other revision.
    pub fn keep_draft(&mut self, cx: &mut Context<Self>) {
        let (Some(model), Some(journal), Some(remote)) =
            (&mut self.model, &self.journal, self.external.clone())
        else {
            return;
        };
        if model.composing() || model.read_only {
            self.message = "Finish composition and enable editing first".into();
            cx.notify();
            return;
        }
        let next = model.revision + 1;
        match journal.resolve_conflict(remote.clone(), next, model.document.clone()) {
            Ok(()) => {
                model.revision = next;
                self.init.document = remote;
                self.external = None;
                self.message.clear();
            }
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }

    /// The conflict banner's "Use the other version": the draft gives way to the other
    /// revision, which is saved back as the note.
    pub fn use_external(&mut self, cx: &mut Context<Self>) {
        let (Some(model), Some(remote)) = (&self.model, self.external.clone()) else {
            return;
        };
        let revision = model.revision;
        let job = self.parse_stored(remote.body.clone(), remote.body_format.clone(), cx);
        cx.spawn(async move |this, cx| {
            let parsed = job.await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .model
                    .as_ref()
                    .is_none_or(|model| model.revision != revision)
                {
                    this.message = "The draft changed meanwhile; choose again.".into();
                } else {
                    match parsed.and_then(|document| this.replace_model(remote, document)) {
                        Ok(()) => this.message.clear(),
                        Err(error) => this.message = error,
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Version history's Restore: the note becomes `body` in one edit that autosaves.
    pub fn restore(
        &mut self,
        body: Arc<str>,
        format: Arc<str>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let job = self.parse_stored(body, format, cx);
        cx.spawn(async move |this, cx| {
            let document = job.await?;
            this.update(cx, |this, cx| {
                let model = this.model.as_mut().ok_or("The note is still loading.")?;
                if model.composing() {
                    return Err("Finish composition before restoring.".to_string());
                }
                let before = model.revision;
                let result = model.replace_all(document);
                this.edited(before, result.clone(), cx);
                result
            })
            .map_err(|_| "The note was closed.".to_string())?
        })
    }

    fn parse_stored(
        &self,
        body: Arc<str>,
        format: Arc<str>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Document, String>> {
        let title = self.title.clone();
        cx.background_executor().spawn(async move {
            let body: Arc<str> = match format.as_ref() {
                "prosemirror_json" => body,
                "markdown" => anlg_tiptap::md_to_tiptap_json(&body)?.to_string().into(),
                format => return Err(format!("Unsupported stored format: {format}")),
            };
            Document::parse(with_title(body, title.as_deref()))
        })
    }

    fn replace_model(
        &mut self,
        remote: DocumentSnapshot,
        document: Document,
    ) -> Result<(), String> {
        let model = self.model.as_ref().ok_or("The note is still loading.")?;
        if model.composing() || model.read_only {
            return Err("Finish composition and enable editing first".into());
        }
        let next = model.revision + 1;
        self.journal
            .as_ref()
            .ok_or(ServiceError::Closed)
            .and_then(|journal| journal.resolve_conflict(remote.clone(), next, document.clone()))
            .map_err(|error| error.to_string())?;
        let mut model = EditorModel::new(document);
        model.revision = next;
        model.read_only = self.read_only;
        self.list_count = model.document.root.children.render_blocks();
        self.list.reset(self.list_count);
        self.model = Some(model);
        self.init.document = remote;
        self.external = None;
        self.projection.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use desktop_runtime::{CancellationToken, Profile, RuntimeHandle, SessionId};
    use futures::executor::block_on;
    use gpui::TestAppContext;
    use serde_json::{Value, json};

    use super::*;
    use crate::contracts::{EditorInit, LaneContext};

    fn wait(cx: &mut TestAppContext, step: &str, done: impl Fn(&mut TestAppContext) -> bool) {
        let start = Instant::now();
        while !done(cx) {
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "editor did not settle: {step}"
            );
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn execute(runtime: &RuntimeHandle, sql: &'static str, params: Vec<Value>) -> Vec<Value> {
        let reply = runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(sql.into(), params)
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
            })
            .unwrap();
        block_on(reply.receive()).unwrap()
    }

    fn paragraph(text: &str) -> String {
        json!({"type": "doc", "content": [
            {"type": "paragraph", "content": [{"type": "text", "text": text}]}
        ]})
        .to_string()
    }

    #[gpui::test]
    fn save_conflicts_resolve_to_the_kept_draft_or_the_other_revision(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(crate::ui::theme::UiFonts::init);
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        execute(
            &runtime,
            "INSERT INTO sessions (id, title) VALUES ('s', 'Note')",
            vec![],
        );
        execute(
            &runtime,
            "INSERT INTO session_documents (id, session_id, kind, body_format, body)
             VALUES ('s', 's', 'note', 'prosemirror_json', ?)",
            vec![json!(paragraph("Original"))],
        );
        let session = SessionId("s".into());
        let document = block_on(
            runtime
                .open_session(session.clone(), CancellationToken::new())
                .unwrap()
                .receive(),
        )
        .unwrap()
        .note
        .unwrap();
        let context = LaneContext {
            runtime: runtime.clone(),
        };
        let window = cx.add_window(|window, cx| {
            let init = EditorInit {
                session_id: session,
                document,
                return_focus: cx.focus_handle(),
            };
            EditorPane::new(context, init, window, cx)
        });
        let editor = window.update(cx, |_, _, cx| cx.entity()).unwrap();
        wait(cx, "load", |cx| {
            editor.read_with(cx, |editor, _| editor.model.is_some())
        });
        let body = || {
            execute(
                &runtime,
                "SELECT body FROM session_documents WHERE id = 's'",
                vec![],
            )[0]["body"]
                .as_str()
                .unwrap()
                .to_owned()
        };

        for (keep, mine, theirs) in [
            (true, "Mine one", "Theirs one"),
            (false, "Mine two", "Theirs two"),
        ] {
            editor
                .update(cx, |editor, cx| {
                    editor.restore(paragraph(mine).into(), "prosemirror_json".into(), cx)
                })
                .detach();
            wait(cx, "edit", |cx| {
                editor.read_with(cx, |editor, _| editor.is_dirty())
            });
            // Another writer replaces the note before the draft's debounced save runs.
            execute(
                &runtime,
                "UPDATE session_documents SET body = ?,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+1 second')
                 WHERE id = 's'",
                vec![json!(paragraph(theirs))],
            );
            wait(cx, "conflict", |cx| {
                editor.read_with(cx, |editor, _| editor.save_conflict())
            });
            // The note's conflict banner explains it; the editor adds no status of its own.
            editor.read_with(cx, |editor, _| assert_eq!(editor.message, ""));
            editor.update(cx, |editor, cx| {
                if keep {
                    editor.keep_draft(cx)
                } else {
                    editor.use_external(cx)
                }
            });
            let expected = if keep { mine } else { theirs };
            wait(cx, "resolve", |cx| {
                body().contains(expected)
                    && editor.read_with(cx, |editor, _| {
                        !editor.is_dirty() && !editor.save_conflict()
                    })
            });
        }
        drop(editor);
        cx.update(|cx| {
            window
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
        block_on(runtime.shutdown()).unwrap();
    }
}
