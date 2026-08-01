use std::time::Duration;

use anlg_ws_client::client::Message;
use bytes::Bytes;

use super::{ListenClientDualInput, TransformedInput, forward_dual_to_single};
use crate::test_utils::{run_dual_test, run_single_test};
use crate::{AssemblyAIAdapter, DeepgramAdapter, ListenClient, RealtimeSttAdapter, SonioxAdapter};

#[derive(Clone, Default)]
struct TestAdapter;

impl RealtimeSttAdapter for TestAdapter {
    fn provider_name(&self) -> &'static str {
        "test"
    }

    fn is_supported_languages(
        &self,
        _languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        true
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(
        &self,
        api_base: &str,
        _params: &owhisper_interface::ListenParams,
        _channels: u8,
    ) -> url::Url {
        api_base.parse().expect("invalid test url")
    }

    fn build_auth_header(&self, _api_key: Option<&str>) -> Option<(&'static str, String)> {
        None
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text("finalize".into())
    }

    fn parse_response(&self, _raw: &str) -> Vec<owhisper_interface::stream::StreamResponse> {
        Vec::new()
    }
}

fn proxy_base() -> String {
    std::env::var("PROXY_URL").unwrap_or_else(|_| "localhost:3001".to_string())
}

#[tokio::test]
async fn malformed_custom_endpoint_returns_provider_configuration_error() {
    let api_base = format!("{}://custom.example/listen", "a".repeat(65));
    let error = ListenClient::builder()
        .adapter::<TestAdapter>()
        .api_base(api_base)
        .build_single()
        .await
        .err()
        .expect("an oversized URI scheme should be rejected");

    assert!(matches!(
        error,
        crate::Error::ProviderConfiguration { provider, .. } if provider == "test"
    ));
}

#[tokio::test]
async fn malformed_direct_endpoint_returns_provider_configuration_error() {
    let api_base = format!("https://api.deepgram.com/{}", "a".repeat(70 * 1024));
    let error = ListenClient::builder()
        .adapter::<DeepgramAdapter>()
        .api_base(api_base)
        .build_single()
        .await
        .err()
        .expect("an oversized direct endpoint should be rejected");

    assert!(matches!(
        error,
        crate::Error::ProviderConfiguration { provider, .. } if provider == "deepgram"
    ));
}

#[tokio::test]
async fn valid_proxy_and_direct_endpoints_still_build() {
    for api_base in [
        "https://api.deepgram.com/v1",
        "https://api.anarlog.so/stt?provider=deepgram",
    ] {
        ListenClient::builder()
            .adapter::<DeepgramAdapter>()
            .api_base(api_base)
            .build_single()
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn forward_dual_to_single_forwards_all_audio_without_dropping() {
    let stream = futures_util::stream::iter(vec![
        ListenClientDualInput::Audio((Bytes::from_static(b"mic-1"), Bytes::from_static(b"spk-1"))),
        ListenClientDualInput::Audio((Bytes::from_static(b"mic-2"), Bytes::from_static(b"spk-2"))),
    ]);
    let (mic_tx, mut mic_rx) = tokio::sync::mpsc::channel(1);
    let (spk_tx, mut spk_rx) = tokio::sync::mpsc::channel(1);

    let task = tokio::spawn(forward_dual_to_single(
        stream,
        mic_tx,
        spk_tx,
        TestAdapter,
        TestAdapter,
    ));

    let Some(TransformedInput::Audio(Message::Binary(first_mic))) = mic_rx.recv().await else {
        panic!("missing first mic frame");
    };
    let Some(TransformedInput::Audio(Message::Binary(first_spk))) = spk_rx.recv().await else {
        panic!("missing first speaker frame");
    };
    let Some(TransformedInput::Audio(Message::Binary(second_mic))) =
        tokio::time::timeout(Duration::from_secs(1), mic_rx.recv())
            .await
            .expect("timed out waiting for second mic frame")
    else {
        panic!("missing second mic frame");
    };
    let Some(TransformedInput::Audio(Message::Binary(second_spk))) =
        tokio::time::timeout(Duration::from_secs(1), spk_rx.recv())
            .await
            .expect("timed out waiting for second speaker frame")
    else {
        panic!("missing second speaker frame");
    };

    assert_eq!(first_mic.as_ref(), b"mic-1");
    assert_eq!(first_spk.as_ref(), b"spk-1");
    assert_eq!(second_mic.as_ref(), b"mic-2");
    assert_eq!(second_spk.as_ref(), b"spk-2");

    let _: () = task.await.expect("forward task panicked");
}

#[tokio::test]
async fn build_single_normalizes_languages_before_initial_message() {
    let client = ListenClient::builder()
        .adapter::<SonioxAdapter>()
        .api_base("https://api.soniox.com")
        .params(owhisper_interface::ListenParams {
            languages: vec![
                "en-US".parse().unwrap(),
                "en-GB".parse().unwrap(),
                anlg_language::ISO639::En.into(),
                "ko-KR".parse().unwrap(),
            ],
            ..Default::default()
        })
        .build_single()
        .await
        .unwrap();

    let msg = client.initial_message.expect("missing initial message");
    let Message::Text(text) = msg else {
        panic!("expected text message");
    };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let hints = json["language_hints"].as_array().unwrap();

    assert_eq!(hints.len(), 2);
    assert_eq!(hints[0].as_str().unwrap(), "en");
    assert_eq!(hints[1].as_str().unwrap(), "ko");
}

#[tokio::test]
#[ignore]
async fn test_proxy_deepgram_single() {
    let client = ListenClient::builder()
        .adapter::<DeepgramAdapter>()
        .api_base(&format!("http://{}", proxy_base()))
        .params(owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_single()
        .await
        .unwrap();

    run_single_test(client, "proxy-deepgram").await;
}

#[tokio::test]
#[ignore]
async fn test_proxy_deepgram_dual() {
    let client = ListenClient::builder()
        .adapter::<DeepgramAdapter>()
        .api_base(&format!("http://{}", proxy_base()))
        .params(owhisper_interface::ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_dual()
        .await
        .unwrap();

    run_dual_test(client, "proxy-deepgram").await;
}

#[tokio::test]
#[ignore]
async fn test_proxy_soniox_single() {
    let client = ListenClient::builder()
        .adapter::<SonioxAdapter>()
        .api_base(&format!("http://{}", proxy_base()))
        .params(owhisper_interface::ListenParams {
            model: Some("stt-v3".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_single()
        .await
        .unwrap();

    run_single_test(client, "proxy-soniox").await;
}

#[tokio::test]
#[ignore]
async fn test_proxy_soniox_dual() {
    let client = ListenClient::builder()
        .adapter::<SonioxAdapter>()
        .api_base(&format!("http://{}", proxy_base()))
        .params(owhisper_interface::ListenParams {
            model: Some("stt-v3".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_dual()
        .await
        .unwrap();

    run_dual_test(client, "proxy-soniox").await;
}

#[tokio::test]
#[ignore]
async fn test_proxy_assemblyai_single() {
    let client = ListenClient::builder()
        .adapter::<AssemblyAIAdapter>()
        .api_base(&format!("http://{}", proxy_base()))
        .params(owhisper_interface::ListenParams {
            model: Some("u3-rt-pro".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_single()
        .await
        .unwrap();

    run_single_test(client, "proxy-assemblyai").await;
}

#[tokio::test]
#[ignore]
async fn test_proxy_assemblyai_dual() {
    let client = ListenClient::builder()
        .adapter::<AssemblyAIAdapter>()
        .api_base(&format!("http://{}", proxy_base()))
        .params(owhisper_interface::ListenParams {
            model: Some("u3-rt-pro".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        })
        .build_dual()
        .await
        .unwrap();

    run_dual_test(client, "proxy-assemblyai").await;
}
