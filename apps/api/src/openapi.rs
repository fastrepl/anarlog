use std::collections::BTreeMap;

use utoipa::openapi::path::{Operation, PathItem};
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityRequirement, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Char AI API",
        version = "1.0.0",
        description = "AI services API for speech-to-text transcription, LLM chat completions, and subscription management"
    ),
    tags(
        (name = "stt", description = "Speech-to-text transcription endpoints"),
        (name = "llm", description = "LLM chat completions endpoints"),
        (name = "calendar", description = "Calendar management"),
        (name = "nango", description = "Integration management via Nango"),
        (name = "subscription", description = "Subscription and trial management")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();

    let stt_doc = hypr_transcribe_proxy::openapi();
    let llm_doc = hypr_llm_proxy::openapi();
    let calendar_doc = with_path_prefix(hypr_api_calendar::openapi(), "/calendar");
    let nango_doc = with_path_prefix(hypr_api_nango::openapi(), "/nango");
    let subscription_doc = with_path_prefix(hypr_api_subscription::openapi(), "/subscription");
    let support_doc = hypr_api_support::openapi();

    doc.merge(stt_doc);
    doc.merge(llm_doc);
    doc.merge(calendar_doc);
    doc.merge(nango_doc);
    doc.merge(subscription_doc);
    doc.merge(support_doc);

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
            || path.starts_with("/subscription")
            || path.starts_with("/nango")
        {
            set_operation_security(item);
        }
    }
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
    #[test]
    fn gen_openapi_json() {
        super::write_openapi_json().unwrap();
    }
}
