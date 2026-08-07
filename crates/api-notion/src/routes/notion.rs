use anlg_api_auth::AuthContext;
use anlg_api_nango::{NangoConnectionState, NangoIntegrationId, Notion};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::blocks::{heading_block, markdown_to_blocks};
use crate::error::{NotionError, Result};

const NOTION_VERSION: &str = "2022-06-28";

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NotionSearchPagesRequest {
    pub connection_id: String,
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct NotionPage {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct NotionPagesResponse {
    pub pages: Vec<NotionPage>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NotionAppendUpdateRequest {
    pub connection_id: String,
    pub page_id: String,
    pub heading: String,
    pub markdown: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct NotionAppendUpdateResponse {
    pub block_count: u32,
}

#[utoipa::path(
    post,
    path = "/search-pages",
    operation_id = "notion_search_pages",
    request_body = NotionSearchPagesRequest,
    responses(
        (status = 200, description = "Notion pages shared with the connected integration", body = NotionPagesResponse),
        (status = 401, description = "Authentication required"),
        (status = 500, description = "Notion connection unavailable"),
    ),
    tag = "notion",
)]
pub async fn search_pages(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<NotionSearchPagesRequest>,
) -> Result<Json<NotionPagesResponse>> {
    let proxy = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Notion::ID,
            &req.connection_id,
        )
        .await?
        .into_proxy();

    let mut body = json!({
        "filter": { "property": "object", "value": "page" },
        "page_size": 20,
    });
    if let Some(query) = req.query.as_deref().map(str::trim)
        && !query.is_empty()
    {
        body["query"] = json!(query);
    }

    let response = proxy
        .post(
            "/v1/search",
            serde_json::to_vec(&body).map_err(|e| NotionError::Notion(e.to_string()))?,
            "application/json",
        )
        .map_err(|e| NotionError::Notion(e.to_string()))?
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(|e| NotionError::Notion(e.to_string()))?
        .error_for_status()
        .map_err(|e| NotionError::Notion(e.to_string()))?;

    let payload: Value = response
        .json()
        .await
        .map_err(|e| NotionError::Notion(e.to_string()))?;

    let pages = payload["results"]
        .as_array()
        .map(|results| results.iter().filter_map(page_from_result).collect())
        .unwrap_or_default();

    Ok(Json(NotionPagesResponse { pages }))
}

#[utoipa::path(
    post,
    path = "/append-update",
    operation_id = "notion_append_update",
    request_body = NotionAppendUpdateRequest,
    responses(
        (status = 200, description = "Update appended to the Notion page", body = NotionAppendUpdateResponse),
        (status = 400, description = "Invalid update payload"),
        (status = 401, description = "Authentication required"),
        (status = 500, description = "Notion connection unavailable"),
    ),
    tag = "notion",
)]
pub async fn append_update(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<NotionAppendUpdateRequest>,
) -> Result<Json<NotionAppendUpdateResponse>> {
    let page_id = req.page_id.trim();
    let heading = req.heading.trim();
    if page_id.is_empty()
        || !page_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(NotionError::BadRequest("invalid page id".to_string()));
    }
    if heading.is_empty() || heading.len() > 300 {
        return Err(NotionError::BadRequest("invalid heading".to_string()));
    }

    let mut children = vec![heading_block(heading)];
    children.extend(markdown_to_blocks(&req.markdown));

    let proxy = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Notion::ID,
            &req.connection_id,
        )
        .await?
        .into_proxy();

    let block_count = children.len() as u32;
    proxy
        .patch(
            format!("/v1/blocks/{page_id}/children"),
            &json!({ "children": children }),
        )
        .map_err(|e| NotionError::Notion(e.to_string()))?
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(|e| NotionError::Notion(e.to_string()))?
        .error_for_status()
        .map_err(|e| NotionError::Notion(e.to_string()))?;

    Ok(Json(NotionAppendUpdateResponse { block_count }))
}

fn page_from_result(result: &Value) -> Option<NotionPage> {
    if result["object"].as_str() != Some("page") {
        return None;
    }
    let id = result["id"].as_str()?.to_string();
    let url = result["url"].as_str().map(str::to_string);
    let title = result["properties"]
        .as_object()
        .and_then(|properties| {
            properties.values().find_map(|property| {
                let fragments = property.get("title")?.as_array()?;
                let title = fragments
                    .iter()
                    .filter_map(|fragment| fragment["plain_text"].as_str())
                    .collect::<String>();
                let title = title.trim().to_string();
                (!title.is_empty()).then_some(title)
            })
        })
        .unwrap_or_else(|| "Untitled".to_string());

    Some(NotionPage { id, title, url })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_page_titles_from_search_results() {
        let result = json!({
            "object": "page",
            "id": "abc-123",
            "url": "https://notion.so/abc",
            "properties": {
                "Name": {
                    "id": "title",
                    "type": "title",
                    "title": [
                        { "plain_text": "Project " },
                        { "plain_text": "Apollo" }
                    ]
                }
            }
        });
        let page = page_from_result(&result).unwrap();
        assert_eq!(page.id, "abc-123");
        assert_eq!(page.title, "Project Apollo");
        assert_eq!(page.url.as_deref(), Some("https://notion.so/abc"));

        let untitled = json!({
            "object": "page",
            "id": "def-456",
            "properties": {}
        });
        assert_eq!(page_from_result(&untitled).unwrap().title, "Untitled");

        let database = json!({ "object": "database", "id": "x" });
        assert!(page_from_result(&database).is_none());
    }
}
