use std::collections::BTreeMap;
use std::path::Path;

use super::SessionParams;
use super::session_span;
use crate::actors::recorder::resolve_final_audio_path;
use crate::{ListenerRuntime, SessionLifecycleEvent};

pub(crate) fn configure_sentry_session_context(params: &SessionParams) {
    sentry::configure_scope(|scope| {
        scope.set_tag("hyprnote.session.id", &params.session_id);
        scope.set_tag(
            "hyprnote.session.type",
            if params.onboarding {
                "onboarding"
            } else {
                "production"
            },
        );

        let mut session_context = BTreeMap::new();
        session_context.insert(
            "hyprnote.session.id".to_string(),
            params.session_id.clone().into(),
        );
        session_context.insert(
            "hyprnote.gen_ai.request.model".to_string(),
            params.model.clone().into(),
        );
        session_context.insert(
            "hyprnote.session.transcription_mode".to_string(),
            format!("{:?}", params.transcription_mode).into(),
        );
        session_context.insert(
            "hyprnote.session.onboarding".to_string(),
            params.onboarding.into(),
        );
        session_context.insert(
            "hyprnote.session.language_codes".to_string(),
            format!("{:?}", params.languages).into(),
        );
        scope.set_context(
            "hyprnote.session",
            sentry::protocol::Context::Other(session_context),
        );
    });
}

pub(crate) fn clear_sentry_session_context() {
    sentry::configure_scope(|scope| {
        scope.remove_tag("hyprnote.session.id");
        scope.remove_tag("hyprnote.session.type");
        scope.remove_context("hyprnote.session");
    });
}

pub(crate) fn emit_session_ended(
    runtime: &dyn ListenerRuntime,
    sessions_base: &Path,
    session_id: &str,
    failure_reason: Option<String>,
    clear_sentry_context: bool,
) {
    let span = session_span(session_id);
    let _guard = span.enter();
    let audio_path = resolve_final_audio_path(sessions_base, session_id)
        .map(|path| path.to_string_lossy().into_owned());

    runtime.emit_lifecycle(SessionLifecycleEvent::Inactive {
        session_id: session_id.to_string(),
        audio_path,
        error: failure_reason.clone(),
    });

    if let Some(reason) = failure_reason {
        tracing::info!(hyprnote.session.stop_reason = %reason, "session_stopped");
    } else {
        tracing::info!("session_stopped");
    }

    if clear_sentry_context {
        clear_sentry_session_context();
    }
}
