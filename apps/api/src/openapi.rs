use std::collections::BTreeMap;

use utoipa::openapi::path::{Operation, PathItem};
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityRequirement, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Anarlog API",
        version = "1.0.0",
        description = "Anarlog cloud services and opt-in hosted meeting access"
    ),
    tags(
        (name = "stt", description = "Speech-to-text transcription endpoints"),
        (name = "llm", description = "LLM chat completions endpoints"),
        (name = "pyannote", description = "Pyannote speaker diarization and voice processing"),
        (name = "calendar", description = "Calendar management"),
        (name = "mail", description = "Mail management"),
        (name = "messenger", description = "Messaging integrations"),
        (name = "notion", description = "Notion integration"),
        (name = "ticket", description = "Ticket management"),
        (name = "nango", description = "Integration management via Nango"),
        (name = "sync", description = "CloudSync credential management"),
        (name = "shared-notes", description = "Public shared-note delivery"),
        (name = "cloud-api", description = "Opt-in hosted access to Anarlog meeting data"),
        (name = "subscription", description = "Subscription and trial management")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();

    let stt_doc = anlg_transcribe_proxy::openapi();
    let llm_doc = anlg_llm_proxy::openapi();
    let pyannote_doc = with_path_prefix(anlg_api_pyannote::openapi(), "/pyannote");
    let calendar_doc = with_path_prefix(anlg_api_calendar::openapi(), "/calendar");
    let mail_doc = with_path_prefix(anlg_api_mail::openapi(), "/mail");
    let messenger_doc = with_path_prefix(anlg_api_messenger::openapi(), "/messenger");
    let notion_doc = with_path_prefix(anlg_api_notion::openapi(), "/notion");
    let ticket_doc = with_path_prefix(anlg_api_ticket::openapi(), "/ticket");
    let nango_doc = with_path_prefix(anlg_api_nango::openapi(), "/nango");
    let subscription_doc = with_path_prefix(anlg_api_subscription::openapi(), "/subscription");
    let sync_doc = with_path_prefix(anlg_api_sync::openapi(), "/sync");
    let shared_notes_doc = anlg_api_sync::shared_notes_openapi();
    let shared_note_recap_doc = anlg_api_sync::shared_note_recap_openapi();
    let cloud_api_doc = anlg_api_cloud::openapi();

    doc.merge(stt_doc);
    doc.merge(llm_doc);
    doc.merge(pyannote_doc);
    doc.merge(calendar_doc);
    doc.merge(mail_doc);
    doc.merge(messenger_doc);
    doc.merge(notion_doc);
    doc.merge(ticket_doc);
    doc.merge(nango_doc);
    doc.merge(subscription_doc);
    doc.merge(sync_doc);
    doc.merge(shared_notes_doc);
    doc.merge(shared_note_recap_doc);
    doc.merge(cloud_api_doc);

    apply_bearer_auth_to_protected_paths(&mut doc);

    doc
}

pub fn write_openapi_json() -> std::io::Result<std::path::PathBuf> {
    let doc = openapi();
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| std::io::Error::other(format!("serialize openapi: {e}")))?;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.gen.json");
    std::fs::write(&path, json)?;
    Ok(path)
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    Http::builder()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("Supabase JWT token"))
                        .build(),
                ),
            );
            components.add_security_scheme(
                "cloud_api_key",
                SecurityScheme::Http(
                    Http::builder()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("anl_...")
                        .description(Some("Anarlog cloud API key"))
                        .build(),
                ),
            );
        }
    }
}

fn with_path_prefix(mut doc: utoipa::openapi::OpenApi, prefix: &str) -> utoipa::openapi::OpenApi {
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return doc;
    }

    let paths = std::mem::take(&mut doc.paths.paths);

    let prefixed: BTreeMap<String, PathItem> = paths
        .into_iter()
        .map(|(path, item)| (format!("{prefix}{path}"), item))
        .collect();

    doc.paths.paths = prefixed;
    doc
}

fn apply_bearer_auth_to_protected_paths(doc: &mut utoipa::openapi::OpenApi) {
    let paths = &mut doc.paths.paths;

    for (path, item) in paths.iter_mut() {
        if path == "/nango/webhook" {
            clear_operation_security(item);
            continue;
        }

        if path.starts_with("/calendar")
            || path.starts_with("/mail")
            || path.starts_with("/ticket")
            || path.starts_with("/subscription")
            || path.starts_with("/nango")
            || path.starts_with("/pyannote")
            || path.starts_with("/sync")
            || path.starts_with("/v1/cloud-api")
            || path.starts_with("/v1/sync-snapshots")
        {
            set_operation_security(item);
        } else if path.starts_with("/v1/meetings") {
            set_cloud_api_key_security(item);
        }
    }
}

fn set_cloud_api_key_security(item: &mut PathItem) {
    let security = Some(vec![SecurityRequirement::new(
        "cloud_api_key",
        Vec::<String>::new(),
    )]);

    with_each_operation(item, |op| {
        op.security = security.clone();
    });
}

fn set_operation_security(item: &mut PathItem) {
    let security = Some(vec![SecurityRequirement::new(
        "bearer_auth",
        Vec::<String>::new(),
    )]);

    with_each_operation(item, |op| {
        op.security = security.clone();
    });
}

fn clear_operation_security(item: &mut PathItem) {
    with_each_operation(item, |op| {
        op.security = None;
    });
}

fn with_each_operation(item: &mut PathItem, mut f: impl FnMut(&mut Operation)) {
    if let Some(op) = item.get.as_mut() {
        f(op);
    }
    if let Some(op) = item.put.as_mut() {
        f(op);
    }
    if let Some(op) = item.post.as_mut() {
        f(op);
    }
    if let Some(op) = item.delete.as_mut() {
        f(op);
    }
    if let Some(op) = item.options.as_mut() {
        f(op);
    }
    if let Some(op) = item.head.as_mut() {
        f(op);
    }
    if let Some(op) = item.patch.as_mut() {
        f(op);
    }
    if let Some(op) = item.trace.as_mut() {
        f(op);
    }
}

#[cfg(test)]
mod tests {
    fn assert_security(
        path: &utoipa::openapi::path::PathItem,
        method: &str,
        security_scheme: &str,
    ) {
        let operation = match method {
            "get" => path.get.as_ref().unwrap(),
            "put" => path.put.as_ref().unwrap(),
            "post" => path.post.as_ref().unwrap(),
            "delete" => path.delete.as_ref().unwrap(),
            _ => unreachable!("unsupported method"),
        };
        let security = operation.security.as_ref().unwrap();

        assert!(security.iter().any(|item| {
            serde_json::to_value(item)
                .unwrap()
                .get(security_scheme)
                .is_some()
        }));
    }

    fn assert_bearer(path: &utoipa::openapi::path::PathItem, method: &str) {
        assert_security(path, method, "bearer_auth");
    }

    fn assert_public(path: &utoipa::openapi::path::PathItem, method: &str) {
        let operation = match method {
            "get" => path.get.as_ref().unwrap(),
            "post" => path.post.as_ref().unwrap(),
            _ => unreachable!("unsupported method"),
        };

        assert!(operation.security.as_ref().is_none_or(Vec::is_empty));
    }

    #[test]
    fn pyannote_paths_are_prefixed_and_protected() {
        let doc = super::openapi();
        assert_bearer(doc.paths.paths.get("/pyannote/v1/diarize").unwrap(), "post");
        assert_bearer(
            doc.paths.paths.get("/pyannote/v1/identify").unwrap(),
            "post",
        );
        assert_bearer(
            doc.paths.paths.get("/pyannote/v1/voiceprint").unwrap(),
            "post",
        );
        assert_bearer(
            doc.paths.paths.get("/pyannote/v1/jobs/{jobId}").unwrap(),
            "get",
        );
        assert_bearer(
            doc.paths.paths.get("/pyannote/v1/media/input").unwrap(),
            "post",
        );
        assert!(!doc.paths.paths.contains_key("/pyannote/v1/jobs"));
        assert!(!doc.paths.paths.contains_key("/pyannote/v1/media/output"));
        assert!(!doc.paths.paths.contains_key("/pyannote/v1/test"));
    }

    #[test]
    fn shared_note_paths_are_public_and_not_sync_prefixed() {
        let doc = super::openapi();

        for (path, method) in [
            ("/shared-notes/public/{slug}", "get"),
            ("/shared-notes/link/{share_id}", "post"),
            ("/shared-notes/links/{link_id}/preview", "get"),
            ("/shared-notes/public/{slug}/handoff", "post"),
            ("/shared-notes/link/{share_id}/handoff", "post"),
            ("/shared-notes/handoffs/claim", "post"),
        ] {
            assert_public(doc.paths.paths.get(path).unwrap(), method);
            assert!(!doc.paths.paths.contains_key(&format!("/sync{path}")));
        }
    }

    #[test]
    fn cloud_api_documents_management_and_connector_auth_separately() {
        let doc = super::openapi();

        let settings = doc.paths.paths.get("/v1/cloud-api/settings").unwrap();
        assert_bearer(settings, "get");
        assert_bearer(settings, "put");
        assert_bearer(
            doc.paths
                .paths
                .get("/v1/sync-snapshots/{session_id}")
                .unwrap(),
            "put",
        );

        for path in [
            "/v1/meetings",
            "/v1/meetings/{meeting_id}",
            "/v1/meetings/{meeting_id}/transcript",
            "/v1/meetings/{meeting_id}/history",
            "/v1/meetings/{meeting_id}/export",
        ] {
            assert_security(doc.paths.paths.get(path).unwrap(), "get", "cloud_api_key");
        }
    }

    #[test]
    fn gen_openapi_json() {
        super::write_openapi_json().unwrap();
    }
}
