use utoipa::OpenApi;
use utoipa::openapi::{
    OpenApi as OpenApiDoc,
    path::{Operation, PathItem},
};

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::request::DiarizeRequest,
    crate::request::DiarizeRequestModel,
    crate::request::IdentifyRequest,
    crate::request::IdentifyRequestModel,
    crate::request::MatchingOptions,
    crate::request::TranscriptionConfiguration,
    crate::request::TranscriptionConfigurationModel,
    crate::request::Voiceprint,
    crate::request::VoiceprintRequest,
    crate::request::VoiceprintRequestModel,
)))]
struct ApiDoc;

pub fn openapi() -> OpenApiDoc {
    let mut doc = hypr_pyannote_cloud::openapi();
    let custom = ApiDoc::openapi();

    doc.paths.paths.retain(|path, _| {
        matches!(
            path.as_str(),
            "/v1/diarize" | "/v1/identify" | "/v1/voiceprint"
        )
    });

    doc.servers = None;
    doc.security = None;

    if let Some(components) = doc.components.as_mut() {
        components.security_schemes.clear();

        if let Some(custom_components) = custom.components {
            components.schemas.extend(custom_components.schemas);
        }
    }

    // Account-wide jobs, shared media helpers, and `/v1/test` validate or expose
    // capabilities of the shared server-side Pyannote account and are not public.
    for item in doc.paths.paths.values_mut() {
        with_each_operation(item, |operation| {
            operation.security = None;
            operation.tags = Some(vec!["pyannote".to_string()]);
        });
    }

    doc
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
    fn normalizes_upstream_pyannote_doc() {
        let doc = super::openapi();
        let diarize = doc.paths.paths.get("/v1/diarize").unwrap();
        let post = diarize.post.as_ref().unwrap();

        assert!(doc.servers.is_none());
        assert!(post.security.is_none());
        assert_eq!(post.tags.as_ref().unwrap(), &vec!["pyannote".to_string()]);
    }

    #[test]
    fn includes_only_public_write_routes() {
        let doc = super::openapi();

        assert!(doc.paths.paths.contains_key("/v1/diarize"));
        assert!(doc.paths.paths.contains_key("/v1/identify"));
        assert!(doc.paths.paths.contains_key("/v1/voiceprint"));
        assert!(!doc.paths.paths.contains_key("/v1/jobs"));
        assert!(!doc.paths.paths.contains_key("/v1/jobs/{jobId}"));
        assert!(!doc.paths.paths.contains_key("/v1/media/input"));
        assert!(!doc.paths.paths.contains_key("/v1/media/output"));
        assert!(!doc.paths.paths.contains_key("/v1/test"));
    }

    #[test]
    fn replaces_public_request_schemas() {
        let doc = super::openapi();
        let schemas = &doc.components.as_ref().unwrap().schemas;

        let diarize = serde_json::to_value(schemas.get("DiarizeRequest").unwrap()).unwrap();
        let identify = serde_json::to_value(schemas.get("IdentifyRequest").unwrap()).unwrap();
        let voiceprint = serde_json::to_value(schemas.get("VoiceprintRequest").unwrap()).unwrap();

        assert!(diarize["properties"].get("webhook").is_none());
        assert!(diarize["properties"].get("webhookStatusOnly").is_none());
        assert!(identify["properties"].get("webhook").is_none());
        assert!(identify["properties"].get("webhookStatusOnly").is_none());
        assert!(voiceprint["properties"].get("webhook").is_none());
        assert!(voiceprint["properties"].get("webhookStatusOnly").is_none());
        assert_eq!(
            identify["required"],
            serde_json::json!(["url", "voiceprints"])
        );
    }
}
