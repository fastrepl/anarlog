use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;

use super::{AnarlogAdapter, STT_PROXY_PROVIDER_NAME};
use crate::adapter::http::mime_type_from_extension;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

impl BatchSttAdapter for AnarlogAdapter {
    fn provider_name(&self) -> &'static str {
        STT_PROXY_PROVIDER_NAME
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        AnarlogAdapter::is_supported_languages_batch(languages, model)
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
        q.append_pair("channels", &params.channels.to_string());
        q.append_pair("sample_rate", &params.sample_rate.to_string());
        for lang in &params.languages {
            q.append_pair("language", &lang.to_string());
        }
        for kw in &params.keywords {
            q.append_pair("keyword", kw);
        }
        if let Some(num_speakers) = params.num_speakers {
            q.append_pair("num_speakers", &num_speakers.to_string());
        }
        if let Some(min_speakers) = params.min_speakers {
            q.append_pair("min_speakers", &min_speakers.to_string());
        }
        if let Some(max_speakers) = params.max_speakers {
            q.append_pair("max_speakers", &max_speakers.to_string());
        }
        if let Some(custom) = &params.custom_query {
            for (key, value) in custom {
                q.append_pair(key, value);
            }
        }
    }

    let file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to open file: {e}")))?;
    let content_length = file
        .metadata()
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to inspect file: {e}")))?
        .len();

    let response = client
        .post(url.to_string())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", mime_type_from_extension(&file_path))
        .header("Content-Length", content_length)
        .body(file)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        Ok(response.json().await?)
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: crate::adapter::http::error_body(response).await,
        })
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::http_client::create_client;

    #[tokio::test]
    async fn forwards_audio_format_to_proxy() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/listen"))
            .and(query_param("channels", "2"))
            .and(query_param("sample_rate", "48000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .mount(&server)
            .await;
        let params = ListenParams {
            channels: 2,
            sample_rate: 48_000,
            ..Default::default()
        };

        do_transcribe_file(
            &create_client(),
            &server.uri(),
            "test-key",
            &params,
            anlg_data::english_1::AUDIO_PATH.into(),
        )
        .await
        .unwrap();
    }
}
