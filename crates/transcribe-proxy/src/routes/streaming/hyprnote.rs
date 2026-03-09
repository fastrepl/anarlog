use std::sync::Arc;

use owhisper_client::{
    AssemblyAIAdapter, Auth, DashScopeAdapter, DeepgramAdapter, ElevenLabsAdapter,
    FireworksAdapter, GladiaAdapter, MistralAdapter, OpenAIAdapter, Provider, RealtimeSttAdapter,
    SonioxAdapter,
};
use owhisper_interface::ListenParams;

use crate::config::SttProxyConfig;
use crate::provider_selector::SelectedProvider;
use crate::query_params::{QueryParams, QueryValue};
use crate::relay::{ChannelSplitProxy, ClientMessageFilter, WebSocketProxy};
use crate::routes::AppState;
use crate::routes::model_resolution::resolve_model_live;

use super::AnalyticsContext;
use super::common::{
    ProxyBuildError, build_on_close_callback, finalize_proxy_builder, parse_param,
};
use super::session::init_session;

fn build_listen_params(params: &QueryParams) -> ListenParams {
    ListenParams {
        model: params.get_first("model").map(|s| s.to_string()),
        languages: params.get_languages(),
        sample_rate: parse_param(params, "sample_rate", 16000),
        channels: parse_param(params, "channels", 1),
        keywords: params.parse_keywords(),
        ..Default::default()
    }
}

fn build_upstream_url_with_adapter(
    provider: Provider,
    api_base: &str,
    params: &ListenParams,
    channels: u8,
) -> url::Url {
    match provider {
        Provider::Deepgram => DeepgramAdapter.build_ws_url(api_base, params, channels),
        Provider::AssemblyAI => AssemblyAIAdapter.build_ws_url(api_base, params, channels),
        Provider::Soniox => SonioxAdapter.build_ws_url(api_base, params, channels),
        Provider::Fireworks => FireworksAdapter.build_ws_url(api_base, params, channels),
        Provider::OpenAI => OpenAIAdapter.build_ws_url(api_base, params, channels),
        Provider::Gladia => GladiaAdapter.build_ws_url(api_base, params, channels),
        Provider::ElevenLabs => ElevenLabsAdapter.build_ws_url(api_base, params, channels),
        Provider::DashScope => DashScopeAdapter.build_ws_url(api_base, params, channels),
        Provider::Mistral => MistralAdapter::default().build_ws_url(api_base, params, channels),
    }
}

fn build_initial_message_with_adapter(
    provider: Provider,
    api_key: Option<&str>,
    params: &ListenParams,
    channels: u8,
) -> Option<String> {
    let msg = match provider {
        Provider::Deepgram => DeepgramAdapter.initial_message(api_key, params, channels),
        Provider::AssemblyAI => AssemblyAIAdapter.initial_message(api_key, params, channels),
        Provider::Soniox => SonioxAdapter.initial_message(api_key, params, channels),
        Provider::Fireworks => FireworksAdapter.initial_message(api_key, params, channels),
        Provider::OpenAI => OpenAIAdapter.initial_message(api_key, params, channels),
        Provider::Gladia => GladiaAdapter.initial_message(api_key, params, channels),
        Provider::ElevenLabs => ElevenLabsAdapter.initial_message(api_key, params, channels),
        Provider::DashScope => DashScopeAdapter.initial_message(api_key, params, channels),
        Provider::Mistral => MistralAdapter::default().initial_message(api_key, params, channels),
    };

    msg.and_then(|m| match m {
        owhisper_client::hypr_ws_client::client::Message::Text(t) => Some(t.to_string()),
        _ => None,
    })
}

fn build_response_transformer(
    provider: Provider,
) -> impl Fn(&str) -> Option<String> + Send + Sync + 'static {
    let mistral_adapter = MistralAdapter::default();
    move |raw: &str| {
        let responses: Vec<owhisper_interface::stream::StreamResponse> = match provider {
            Provider::Deepgram => DeepgramAdapter.parse_response(raw),
            Provider::AssemblyAI => AssemblyAIAdapter.parse_response(raw),
            Provider::Soniox => SonioxAdapter.parse_response(raw),
            Provider::Fireworks => FireworksAdapter.parse_response(raw),
            Provider::OpenAI => OpenAIAdapter.parse_response(raw),
            Provider::Gladia => GladiaAdapter.parse_response(raw),
            Provider::ElevenLabs => ElevenLabsAdapter.parse_response(raw),
            Provider::DashScope => DashScopeAdapter.parse_response(raw),
            Provider::Mistral => mistral_adapter.parse_response(raw),
        };

        if responses.is_empty() {
            return None;
        }

        if responses.len() == 1 {
            return serde_json::to_string(&responses[0]).ok();
        }

        serde_json::to_string(&responses).ok()
    }
}

fn build_client_message_filter(provider: Provider) -> ClientMessageFilter {
    Arc::new(move |text: String| {
        let msg = match serde_json::from_str::<owhisper_interface::ControlMessage>(&text) {
            Ok(msg) => msg,
            Err(_) => return Some(text),
        };
        provider.translate_control_message(&msg)
    })
}

pub enum StreamingProxy {
    Single(WebSocketProxy),
    ChannelSplit(ChannelSplitProxy),
}

fn build_proxy_with_adapter(
    selected: &SelectedProvider,
    client_params: &QueryParams,
    config: &SttProxyConfig,
    api_base: &str,
    analytics_ctx: AnalyticsContext,
) -> Result<StreamingProxy, crate::ProxyError> {
    let provider = selected.provider();
    let mut listen_params = build_listen_params(client_params);
    let channels: u8 = parse_param(client_params, "channels", 1);
    if channels > 2 {
        return Err(crate::ProxyError::InvalidRequest(
            "channels must be 1 or 2".to_string(),
        ));
    }

    resolve_model_live(provider, &mut listen_params);

    if channels > 1 && !provider.supports_native_multichannel() {
        return build_channel_split_proxy(
            selected,
            config,
            api_base,
            &listen_params,
            analytics_ctx,
        );
    }

    let upstream_url =
        build_upstream_url_with_adapter(provider, api_base, &listen_params, channels);

    let initial_message = build_initial_message_with_adapter(
        provider,
        Some(selected.api_key()),
        &listen_params,
        channels,
    );

    let filter = build_client_message_filter(provider);
    let mut builder = WebSocketProxy::builder()
        .upstream_url(upstream_url.as_str())
        .connect_timeout(config.connect_timeout)
        .control_message_types(provider.control_message_types())
        .response_transformer(build_response_transformer(provider))
        .client_message_filter(filter)
        .apply_auth(selected);

    if let Some(msg) = initial_message {
        builder = builder.initial_message(msg);
    }

    let proxy = finalize_proxy_builder!(builder, provider, config, analytics_ctx)?;
    Ok(StreamingProxy::Single(proxy))
}

fn build_channel_split_proxy(
    selected: &SelectedProvider,
    config: &SttProxyConfig,
    api_base: &str,
    listen_params: &ListenParams,
    analytics_ctx: AnalyticsContext,
) -> Result<StreamingProxy, crate::ProxyError> {
    let provider = selected.provider();
    let upstream_url = build_upstream_url_with_adapter(provider, api_base, listen_params, 1);

    let initial_message =
        build_initial_message_with_adapter(provider, Some(selected.api_key()), listen_params, 1);

    let uri: axum::http::Uri =
        upstream_url
            .as_str()
            .parse()
            .map_err(|e: axum::http::uri::InvalidUri| {
                crate::ProxyError::InvalidRequest(e.to_string())
            })?;

    let mut request = tokio_tungstenite::tungstenite::ClientRequestBuilder::new(uri);
    if let Some((name, value)) = provider.build_auth_header(selected.api_key()) {
        request = request.with_header(name, value);
    }

    let initial_msg: Option<crate::relay::InitialMessage> = initial_message.map(Arc::new);
    let response_transformer: Option<crate::relay::ResponseTransformer> =
        Some(Arc::new(build_response_transformer(provider)));
    let on_close = build_on_close_callback(config, provider, &analytics_ctx);

    let filter = build_client_message_filter(provider);
    Ok(StreamingProxy::ChannelSplit(
        ChannelSplitProxy::new(
            request,
            initial_msg,
            response_transformer,
            config.connect_timeout,
            on_close,
        )
        .with_client_message_filter(filter),
    ))
}

fn build_session_channel_split_proxy(
    selected: &SelectedProvider,
    config: &SttProxyConfig,
    url_mic: &str,
    url_spk: &str,
    analytics_ctx: AnalyticsContext,
) -> Result<StreamingProxy, crate::ProxyError> {
    let provider = selected.provider();

    let mic_uri: axum::http::Uri = url_mic.parse().map_err(|e: axum::http::uri::InvalidUri| {
        crate::ProxyError::InvalidRequest(e.to_string())
    })?;
    let spk_uri: axum::http::Uri = url_spk.parse().map_err(|e: axum::http::uri::InvalidUri| {
        crate::ProxyError::InvalidRequest(e.to_string())
    })?;

    let mic_request = tokio_tungstenite::tungstenite::ClientRequestBuilder::new(mic_uri);
    let spk_request = tokio_tungstenite::tungstenite::ClientRequestBuilder::new(spk_uri);

    let response_transformer: Option<crate::relay::ResponseTransformer> =
        Some(Arc::new(build_response_transformer(provider)));
    let on_close = build_on_close_callback(config, provider, &analytics_ctx);

    let filter = build_client_message_filter(provider);
    Ok(StreamingProxy::ChannelSplit(
        ChannelSplitProxy::with_split_requests(
            mic_request,
            spk_request,
            None,
            response_transformer,
            config.connect_timeout,
            on_close,
        )
        .with_client_message_filter(filter),
    ))
}

fn build_proxy_with_url_and_transformer(
    selected: &SelectedProvider,
    upstream_url: &str,
    config: &SttProxyConfig,
    analytics_ctx: AnalyticsContext,
) -> Result<StreamingProxy, crate::ProxyError> {
    let provider = selected.provider();
    let filter = build_client_message_filter(provider);
    let builder = WebSocketProxy::builder()
        .upstream_url(upstream_url)
        .connect_timeout(config.connect_timeout)
        .control_message_types(provider.control_message_types())
        .response_transformer(build_response_transformer(provider))
        .client_message_filter(filter)
        .apply_auth(selected);

    let proxy = finalize_proxy_builder!(builder, provider, config, analytics_ctx)?;
    Ok(StreamingProxy::Single(proxy))
}

pub async fn build_proxy(
    state: &AppState,
    selected: &SelectedProvider,
    params: &QueryParams,
    analytics_ctx: AnalyticsContext,
) -> Result<StreamingProxy, ProxyBuildError> {
    let provider = selected.provider();
    let api_base = selected
        .upstream_url()
        .unwrap_or(provider.default_api_base());

    match provider.auth() {
        Auth::SessionInit { header_name } => {
            if selected.upstream_url().is_some() {
                Ok(build_proxy_with_adapter(
                    selected,
                    params,
                    &state.config,
                    api_base,
                    analytics_ctx,
                )?)
            } else {
                let channels: u8 = parse_param(params, "channels", 1);
                if channels > 1 && !provider.supports_native_multichannel() {
                    let mut mono_params = params.clone();
                    mono_params.insert("channels".to_string(), QueryValue::Single("1".to_string()));
                    let (url_mic, url_spk) = tokio::try_join!(
                        init_session(state, selected, header_name, &mono_params),
                        init_session(state, selected, header_name, &mono_params),
                    )
                    .map_err(ProxyBuildError::SessionInitFailed)?;
                    Ok(build_session_channel_split_proxy(
                        selected,
                        &state.config,
                        &url_mic,
                        &url_spk,
                        analytics_ctx,
                    )?)
                } else {
                    let url = init_session(state, selected, header_name, params)
                        .await
                        .map_err(ProxyBuildError::SessionInitFailed)?;
                    Ok(build_proxy_with_url_and_transformer(
                        selected,
                        &url,
                        &state.config,
                        analytics_ctx,
                    )?)
                }
            }
        }
        _ => Ok(build_proxy_with_adapter(
            selected,
            params,
            &state.config,
            api_base,
            analytics_ctx,
        )?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_params::QueryValue;
    use hypr_language::ISO639;

    #[test]
    fn test_build_listen_params_basic() {
        let mut params = QueryParams::default();
        params.insert(
            "model".to_string(),
            QueryValue::Single("nova-3".to_string()),
        );
        params.insert("language".to_string(), QueryValue::Single("en".to_string()));
        params.insert(
            "sample_rate".to_string(),
            QueryValue::Single("16000".to_string()),
        );
        params.insert("channels".to_string(), QueryValue::Single("1".to_string()));

        let listen_params = build_listen_params(&params);

        assert_eq!(listen_params.model, Some("nova-3".to_string()));
        assert_eq!(listen_params.languages.len(), 1);
        assert_eq!(listen_params.languages[0].iso639(), ISO639::En);
        assert_eq!(listen_params.sample_rate, 16000);
        assert_eq!(listen_params.channels, 1);
    }

    #[test]
    fn test_build_listen_params_with_keywords() {
        let mut params = QueryParams::default();
        params.insert(
            "keyword".to_string(),
            QueryValue::Multi(vec!["Hyprnote".to_string(), "transcription".to_string()]),
        );

        let listen_params = build_listen_params(&params);

        assert_eq!(listen_params.keywords.len(), 2);
        assert!(listen_params.keywords.contains(&"Hyprnote".to_string()));
        assert!(
            listen_params
                .keywords
                .contains(&"transcription".to_string())
        );
    }

    #[test]
    fn test_build_listen_params_default_values() {
        let params = QueryParams::default();
        let listen_params = build_listen_params(&params);

        assert_eq!(listen_params.model, None);
        assert!(listen_params.languages.is_empty());
        assert_eq!(listen_params.sample_rate, 16000);
        assert_eq!(listen_params.channels, 1);
        assert!(listen_params.keywords.is_empty());
    }

    #[test]
    fn test_build_upstream_url_deepgram() {
        let params = ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            sample_rate: 16000,
            channels: 1,
            ..Default::default()
        };

        let url = build_upstream_url_with_adapter(
            Provider::Deepgram,
            "https://api.deepgram.com/v1",
            &params,
            1,
        );

        assert!(url.as_str().contains("deepgram.com"));
        assert!(url.as_str().contains("model=nova-3"));
    }

    #[test]
    fn test_build_upstream_url_soniox() {
        let params = ListenParams {
            model: Some("stt-rt-v3".to_string()),
            languages: vec![ISO639::En.into()],
            sample_rate: 16000,
            channels: 1,
            ..Default::default()
        };

        let url =
            build_upstream_url_with_adapter(Provider::Soniox, "https://api.soniox.com", &params, 1);

        assert!(url.as_str().contains("soniox.com"));
    }

    #[test]
    fn test_build_initial_message_soniox() {
        let params = ListenParams {
            model: Some("stt-rt-v3".to_string()),
            languages: vec![ISO639::En.into()],
            sample_rate: 16000,
            channels: 1,
            ..Default::default()
        };

        let initial_msg =
            build_initial_message_with_adapter(Provider::Soniox, Some("test-key"), &params, 1);

        assert!(initial_msg.is_some());
        let msg = initial_msg.unwrap();
        assert!(msg.contains("api_key"));
        assert!(msg.contains("test-key"));
    }

    #[test]
    fn test_build_initial_message_deepgram_none() {
        let params = ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let initial_msg =
            build_initial_message_with_adapter(Provider::Deepgram, Some("test-key"), &params, 1);

        assert!(initial_msg.is_none());
    }

    #[test]
    fn test_response_transformer_deepgram() {
        let transformer = build_response_transformer(Provider::Deepgram);

        let deepgram_response = r#"{
            "type": "Results",
            "channel_index": [0, 1],
            "duration": 1.0,
            "start": 0.0,
            "is_final": true,
            "speech_final": true,
            "from_finalize": false,
            "channel": {
                "alternatives": [{
                    "transcript": "hello world",
                    "confidence": 0.95,
                    "words": []
                }]
            },
            "metadata": {
                "request_id": "test",
                "model_uuid": "test",
                "model_info": {
                    "name": "nova-3",
                    "version": "1",
                    "arch": "test"
                }
            }
        }"#;

        let result = transformer(deepgram_response);
        assert!(result.is_some());

        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed["type"], "Results");
    }

    #[test]
    fn test_response_transformer_empty_response() {
        let transformer = build_response_transformer(Provider::Soniox);

        let result = transformer("{}");
        assert!(result.is_none());
    }

    #[test]
    fn test_client_message_filter_deepgram_identity() {
        let filter = build_client_message_filter(Provider::Deepgram);
        assert_eq!(
            filter(r#"{"type":"KeepAlive"}"#.to_string()),
            Some(r#"{"type":"KeepAlive"}"#.to_string())
        );
        assert_eq!(filter(r#"{"type":"CloseStream"}"#.to_string()), None);
        assert_eq!(
            filter(r#"{"type":"Finalize"}"#.to_string()),
            Some(r#"{"type":"Finalize"}"#.to_string())
        );
    }

    #[test]
    fn test_client_message_filter_soniox_translates_control_messages() {
        let filter = build_client_message_filter(Provider::Soniox);

        assert_eq!(filter(r#"{"type":"CloseStream"}"#.to_string()), None);
        assert_eq!(
            filter(r#"{"type":"KeepAlive"}"#.to_string()),
            Some(r#"{"type":"keepalive"}"#.to_string())
        );
        assert_eq!(
            filter(r#"{"type":"Finalize"}"#.to_string()),
            Some(r#"{"type":"finalize"}"#.to_string())
        );
    }

    #[test]
    fn test_client_message_filter_assemblyai_translates_finalize() {
        let filter = build_client_message_filter(Provider::AssemblyAI);
        assert_eq!(filter(r#"{"type":"KeepAlive"}"#.to_string()), None);
        assert_eq!(
            filter(r#"{"type":"Finalize"}"#.to_string()),
            Some(r#"{"type":"Terminate"}"#.to_string())
        );
    }

    #[test]
    fn test_client_message_filter_non_json_passthrough() {
        let filter = build_client_message_filter(Provider::Soniox);
        assert_eq!(filter("not json".to_string()), Some("not json".to_string()));
    }

    #[test]
    fn test_resolve_model_clears_meta_model_for_soniox() {
        let mut params = ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![ISO639::Ko.into(), ISO639::En.into()],
            sample_rate: 16000,
            ..Default::default()
        };

        resolve_model_live(Provider::Soniox, &mut params);
        assert_eq!(params.model, None);
    }

    #[test]
    fn test_resolve_model_resolves_meta_model_for_deepgram() {
        let mut params = ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![ISO639::En.into()],
            sample_rate: 16000,
            ..Default::default()
        };

        resolve_model_live(Provider::Deepgram, &mut params);
        assert!(params.model.is_some());
        assert_ne!(params.model.as_deref(), Some("cloud"));
    }

    #[test]
    fn test_resolve_model_preserves_explicit_model() {
        let mut params = ListenParams {
            model: Some("nova-3".to_string()),
            languages: vec![ISO639::En.into()],
            sample_rate: 16000,
            ..Default::default()
        };

        resolve_model_live(Provider::Deepgram, &mut params);
        assert_eq!(params.model, Some("nova-3".to_string()));
    }

    #[test]
    fn test_resolve_model_none_triggers_resolution() {
        let mut params = ListenParams {
            model: None,
            languages: vec![ISO639::En.into()],
            sample_rate: 16000,
            ..Default::default()
        };

        resolve_model_live(Provider::Deepgram, &mut params);
        assert!(params.model.is_some());
    }
}
