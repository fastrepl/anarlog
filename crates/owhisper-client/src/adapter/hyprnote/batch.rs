use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;

use super::HyprnoteAdapter;
use crate::adapter::http::mime_type_from_extension;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

impl BatchSttAdapter for HyprnoteAdapter {
    fn provider_name(&self) -> &'static str {
        "hyprnote"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        HyprnoteAdapter::is_supported_languages_batch(languages, model)
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a> {
        let path = file_path.as_ref().to_path_buf();
        Box::pin(async move { do_transcribe_file(client, api_base, api_key, params, path).await })
    }
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let mut url: url::Url = api_base
        .parse()
        .map_err(|e: url::ParseError| Error::AudioProcessing(e.to_string()))?;
    append_path_if_missing(&mut url, "listen");
    {
        let mut q = url.query_pairs_mut();
        if let Some(model) = &params.model {
            q.append_pair("model", model);
        }
        for lang in &params.languages {
            q.append_pair("language", &lang.to_string());
        }
        for kw in &params.keywords {
            q.append_pair("keyword", kw);
        }
        if let Some(custom) = &params.custom_query {
            for (key, value) in custom {
                q.append_pair(key, value);
            }
        }
    }

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to read file: {e}")))?;

    let response = client
        .post(url.to_string())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", mime_type_from_extension(&file_path))
        .body(bytes)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        Ok(response.json().await?)
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}
