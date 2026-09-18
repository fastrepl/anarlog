use super::WisprFlowAdapter;
use crate::{
    adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware},
    error::Error,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use owhisper_interface::{
    ListenParams,
    batch::{Alternatives, Channel, Response, Results},
};
use serde_json::json;
use std::{io::Cursor, path::Path};

impl BatchSttAdapter for WisprFlowAdapter {
    fn provider_name(&self) -> &'static str {
        "wisprflow"
    }
    fn is_supported_languages(&self, _: &[anlg_language::Language], _: Option<&str>) -> bool {
        true
    }
    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        base: &'a str,
        key: &'a str,
        params: &'a ListenParams,
        path: P,
    ) -> BatchFuture<'a> {
        let path = path.as_ref().to_owned();
        Box::pin(async move {
            let audio = tokio::task::spawn_blocking(move || encode(&path)).await??;
            let response = client.post(Self::endpoint(base, "/api/v1/dash/api")?)
                .bearer_auth(key).json(&json!({
                    "audio": STANDARD.encode(audio),
                    "language": params.languages.iter().map(|l| l.iso639_code().to_string()).collect::<Vec<_>>(),
                    "context": {"dictionary_context": params.keywords},
                })).send().await?;
            let status = response.status();
            if !status.is_success() {
                return Err(Error::UnexpectedStatus {
                    status,
                    body: crate::adapter::http::error_body(response).await,
                });
            }
            #[derive(serde::Deserialize)]
            struct Transcript {
                text: String,
            }
            let text = response.json::<Transcript>().await?.text;
            Ok(Response {
                metadata: json!({}),
                results: Results {
                    channels: vec![Channel {
                        alternatives: vec![Alternatives {
                            transcript: text,
                            confidence: 1.0,
                            words: vec![],
                        }],
                    }],
                },
            })
        })
    }
}

fn encode(path: &Path) -> Result<Vec<u8>, Error> {
    use anlg_audio_utils::Source;
    let source = anlg_audio_utils::source_from_path(path)
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;
    let channels = usize::from(u16::from(source.channels()));
    let samples = anlg_audio_utils::resample_audio(source, 16_000)
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;
    let mono: Vec<f32> = anlg_audio_utils::mono_frames(samples.into_iter(), channels).collect();
    if mono.len() > 16_000 * 360 {
        return Err(Error::AudioProcessing(
            "Wispr Flow accepts at most six minutes per request.".into(),
        ));
    }
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;
        for sample in anlg_audio_utils::f32_to_i16_samples(&mono) {
            writer
                .write_sample(sample)
                .map_err(|e| Error::AudioProcessing(e.to_string()))?;
        }
        writer
            .finalize()
            .map_err(|e| Error::AudioProcessing(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };
    #[tokio::test]
    async fn uploads_mono_pcm_wav_and_reads_polished_text() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = hound::WavWriter::create(
            file.path(),
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..1600 {
            writer.write_sample(123_i16).unwrap();
        }
        writer.finalize().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/dash/api"))
            .and(header("authorization", "Bearer test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"text":"Hello."})))
            .mount(&server)
            .await;
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
        let response = WisprFlowAdapter::default()
            .transcribe_file(
                &client,
                &server.uri(),
                "test",
                &ListenParams::default(),
                file.path(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.results.channels[0].alternatives[0].transcript,
            "Hello."
        );
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        let wav = STANDARD.decode(body["audio"].as_str().unwrap()).unwrap();
        let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.duration(), 1600);
    }
}
