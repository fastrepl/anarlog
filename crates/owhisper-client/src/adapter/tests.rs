use super::*;

#[test]
fn test_normalize_languages_deduplicates_same_base() {
    use anlg_language::{ISO639, Language};

    let en: Language = ISO639::En.into();
    let en_gb = Language::with_region(ISO639::En, "GB");
    let es: Language = ISO639::Es.into();

    let result = normalize_languages(&[en.clone(), en_gb.clone(), es.clone()]);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].iso639(), ISO639::En);
    assert_eq!(result[0].region(), None);
    assert_eq!(result[1].iso639(), ISO639::Es);
}

#[test]
fn test_normalize_languages_prefers_base_over_regional() {
    use anlg_language::{ISO639, Language};

    let en_gb = Language::with_region(ISO639::En, "GB");
    let en: Language = ISO639::En.into();

    let result = normalize_languages(&[en_gb.clone(), en.clone()]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].iso639(), ISO639::En);
    assert_eq!(result[0].region(), None);
}

#[test]
fn test_normalize_languages_keeps_regional_if_no_base() {
    use anlg_language::{ISO639, Language};

    let en_gb = Language::with_region(ISO639::En, "GB");
    let es: Language = ISO639::Es.into();

    let result = normalize_languages(&[en_gb.clone(), es.clone()]);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].iso639(), ISO639::En);
    assert_eq!(result[0].region(), Some("GB"));
    assert_eq!(result[1].iso639(), ISO639::Es);
}

#[test]
fn test_normalize_languages_multiple_variants() {
    use anlg_language::{ISO639, Language};

    let en_us = Language::with_region(ISO639::En, "US");
    let en_gb = Language::with_region(ISO639::En, "GB");
    let en: Language = ISO639::En.into();

    let result = normalize_languages(&[en_us.clone(), en_gb.clone(), en.clone()]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].iso639(), ISO639::En);
    assert_eq!(result[0].region(), None);
}

#[test]
fn test_simple_documented_language_codes_collapses_variants() {
    let result = simple_documented_language_codes(["zh", "zh-CN", "zh-Hans", "en-US", "en-GB"]);

    assert_eq!(result, vec!["en".to_string(), "zh".to_string()]);
}

#[test]
fn test_simple_documented_language_codes_canonicalizes_aliases() {
    let result = simple_documented_language_codes(["jw", "jv"]);

    assert_eq!(result, vec!["jv".to_string()]);
}

#[test]
fn test_documented_language_codes_are_menu_safe() {
    let result = documented_language_codes_live();

    assert!(result.contains(&"en".to_string()));
    assert!(result.contains(&"zh".to_string()));
    assert!(result.iter().all(|code| !code.contains("-")));
    assert!(!result.contains(&"jw".to_string()));
}

#[test]
fn test_is_anarlog_proxy() {
    assert!(is_anarlog_proxy("https://api.anarlog.so/stt"));
    assert!(is_anarlog_proxy("https://api.anarlog.so"));
    assert!(is_anarlog_proxy("http://localhost:3001/stt"));
    assert!(is_anarlog_proxy("http://127.0.0.1:3001/stt"));

    assert!(!is_anarlog_proxy("https://api.hyprnote.com/stt"));
    assert!(!is_anarlog_proxy("https://api.char.com/stt"));
    assert!(!is_anarlog_proxy("https://notchar.com/stt"));
    assert!(!is_anarlog_proxy("https://api.deepgram.com"));
    assert!(!is_anarlog_proxy("http://localhost:50060/v1"));
}

#[test]
fn test_is_local_argmax() {
    assert!(is_local_argmax("http://localhost:50060/v1"));
    assert!(is_local_argmax("http://127.0.0.1:50060/v1"));

    assert!(!is_local_argmax("https://api.anarlog.so/stt"));
    assert!(!is_local_argmax("http://localhost:3001/stt"));
    assert!(!is_local_argmax("https://api.deepgram.com"));
}

#[test]
fn test_adapter_kind_from_url_and_languages() {
    use anlg_language::ISO639::*;

    let cases: &[(&str, &[anlg_language::ISO639], Option<&str>, AdapterKind)] = &[
        // AnarlogCloud - always routes to Anarlog adapter (proxy owns provider selection)
        (
            "https://api.anarlog.so/stt",
            &[En],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[En],
            Some("cloud"),
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[En, Ko],
            Some("cloud"),
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[Zh],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[Ja],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[Ar],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[De],
            None,
            AdapterKind::Anarlog,
        ),
        // AnarlogCloud - multi-language
        (
            "https://api.anarlog.so/stt",
            &[En, Es],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[En, Ko],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[Ko, En],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "https://api.anarlog.so/stt",
            &[En, De],
            None,
            AdapterKind::Anarlog,
        ),
        // localhost proxy
        (
            "http://localhost:3001/stt",
            &[En],
            None,
            AdapterKind::Anarlog,
        ),
        (
            "http://localhost:3001/stt",
            &[Ar],
            None,
            AdapterKind::Anarlog,
        ),
        // localhost argmax
        (
            "http://localhost:50060/v1",
            &[En],
            None,
            AdapterKind::Argmax,
        ),
        (
            "https://openrouter.ai/api/v1",
            &[En],
            Some("openai/gpt-4o-mini-transcribe"),
            AdapterKind::OpenRouter,
        ),
        (
            "https://api.siliconflow.cn/v1",
            &[Zh],
            Some("FunAudioLLM/SenseVoiceSmall"),
            AdapterKind::SiliconFlow,
        ),
        (
            "https://api.z.ai/api/paas/v4",
            &[En],
            Some("glm-asr-2512"),
            AdapterKind::Zai,
        ),
    ];

    for (url, langs, model, expected) in cases {
        let langs: Vec<anlg_language::Language> = langs.iter().map(|l| (*l).into()).collect();
        assert_eq!(
            AdapterKind::from_url_and_languages(url, &langs, *model),
            *expected,
            "url={url}, langs={langs:?}, model={model:?}"
        );
    }
}

#[test]
fn test_has_live_mode() {
    let live = [
        AdapterKind::Deepgram,
        AdapterKind::Soniox,
        AdapterKind::AssemblyAI,
        AdapterKind::Gladia,
        AdapterKind::Fireworks,
        AdapterKind::OpenAI,
        AdapterKind::ElevenLabs,
        AdapterKind::DashScope,
        AdapterKind::Mistral,
        AdapterKind::Xai,
        AdapterKind::Anarlog,
    ];
    for kind in live {
        assert!(kind.has_live_mode(), "{kind:?} should support live mode");
    }

    let batch_only = [
        AdapterKind::AquaVoice,
        AdapterKind::Argmax,
        AdapterKind::Pyannote,
        AdapterKind::Cohere,
        AdapterKind::AwsTranscribe,
        AdapterKind::AzureSpeech,
        AdapterKind::GoogleCloud,
        AdapterKind::Groq,
        AdapterKind::OpenRouter,
        AdapterKind::SiliconFlow,
        AdapterKind::Zai,
        AdapterKind::RevAi,
        AdapterKind::Speechmatics,
        AdapterKind::Together,
    ];
    for kind in batch_only {
        assert!(
            !kind.has_live_mode(),
            "{kind:?} should not support live mode"
        );
    }
}

#[test]
fn test_build_proxy_ws_url() {
    let cases: &[(&str, Option<(&str, Vec<(&str, &str)>)>)] = &[
        ("", None),
        ("https://api.deepgram.com", None),
        ("https://api.soniox.com", None),
        ("https://api.fireworks.ai", None),
        ("https://api.assemblyai.com", None),
        (
            "https://api.anarlog.so/stt?provider=soniox",
            Some((
                "wss://api.anarlog.so/stt/listen",
                vec![("provider", "soniox")],
            )),
        ),
        (
            "https://api.anarlog.so/stt?provider=deepgram",
            Some((
                "wss://api.anarlog.so/stt/listen",
                vec![("provider", "deepgram")],
            )),
        ),
        (
            "https://api.anarlog.so/stt?provider=anarlog",
            Some((
                "wss://api.anarlog.so/stt/listen",
                vec![("provider", "anarlog")],
            )),
        ),
        (
            "https://api.anarlog.so/stt/listen?provider=deepgram",
            Some((
                "wss://api.anarlog.so/stt/listen",
                vec![("provider", "deepgram")],
            )),
        ),
        (
            "https://api.anarlog.so/stt/some/path?provider=fireworks",
            Some((
                "wss://api.anarlog.so/stt/some/path/listen",
                vec![("provider", "fireworks")],
            )),
        ),
        (
            "http://localhost:8787/stt?provider=soniox",
            Some((
                "ws://localhost:8787/stt/listen",
                vec![("provider", "soniox")],
            )),
        ),
        (
            "http://localhost:8787/stt/listen?provider=deepgram",
            Some((
                "ws://localhost:8787/stt/listen",
                vec![("provider", "deepgram")],
            )),
        ),
        (
            "http://127.0.0.1:8787/stt?provider=assemblyai",
            Some((
                "ws://127.0.0.1:8787/stt/listen",
                vec![("provider", "assemblyai")],
            )),
        ),
    ];

    for (input, expected) in cases {
        let result = build_proxy_ws_url(input);
        match (result, expected) {
            (None, None) => {}
            (Some((url, params)), Some((expected_url, expected_params))) => {
                assert_eq!(url.as_str(), *expected_url, "input: {}", input);
                assert_eq!(
                    params,
                    expected_params
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>(),
                    "input: {}",
                    input
                );
            }
            (result, expected) => {
                panic!(
                    "input: {}, expected: {:?}, got: {:?}",
                    input, expected, result
                );
            }
        }
    }
}

#[test]
fn test_anarlog_proxy_always_selects_anarlog_adapter() {
    use anlg_language::ISO639::*;

    let proxy_urls = &[
        "https://api.anarlog.so/stt",
        "http://localhost:3001/stt",
        "http://127.0.0.1:3001/stt",
    ];

    let language_combos: &[&[anlg_language::ISO639]] = &[&[En], &[Ko], &[En, De], &[En, Ko], &[Ar]];

    for url in proxy_urls {
        for langs in language_combos {
            let langs: Vec<anlg_language::Language> = langs.iter().map(|l| (*l).into()).collect();
            assert_eq!(
                AdapterKind::from_url_and_languages(url, &langs, Some("cloud")),
                AdapterKind::Anarlog,
                "proxy URL should always select Anarlog adapter regardless of languages: url={url}, langs={langs:?}"
            );
        }
    }
}

#[test]
fn test_anarlog_cloud_adapter_supports_all_languages() {
    use anlg_language::ISO639::*;

    let combos: &[&[anlg_language::ISO639]] = &[&[En], &[Ko], &[Ar], &[En, De], &[En, Ko], &[Zh]];

    for langs in combos {
        let langs: Vec<anlg_language::Language> = langs.iter().map(|l| (*l).into()).collect();
        assert!(
            AdapterKind::Anarlog.is_supported_languages_live(&langs, Some("cloud")),
            "Anarlog adapter should support all languages: {langs:?}"
        );
    }
}

#[test]
fn test_anarlog_soniqo_live_limits_parakeet_languages() {
    use anlg_language::ISO639::*;

    let fr: Vec<anlg_language::Language> = vec![Fr.into()];
    let ko: Vec<anlg_language::Language> = vec![Ko.into()];

    assert!(
        AdapterKind::Anarlog.is_supported_languages_live(&fr, Some("soniqo-parakeet-streaming"))
    );
    assert!(
        !AdapterKind::Anarlog.is_supported_languages_live(&ko, Some("soniqo-parakeet-streaming"))
    );
}

#[test]
fn test_anarlog_soniqo_live_rejects_batch_only_models() {
    use anlg_language::ISO639::*;

    let fr: Vec<anlg_language::Language> = vec![Fr.into()];

    assert!(!AdapterKind::Anarlog.is_supported_languages_live(&fr, Some("soniqo-parakeet-batch")));
    assert!(!AdapterKind::Anarlog.is_supported_languages_live(&fr, Some("soniqo-qwen3-small")));
}

#[test]
fn test_direct_provider_urls_not_affected() {
    use anlg_language::ISO639::*;

    let en: Vec<anlg_language::Language> = vec![En.into()];
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.deepgram.com/v1", &en, None),
        AdapterKind::Deepgram,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.soniox.com", &en, None),
        AdapterKind::Soniox,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.pyannote.ai", &en, None),
        AdapterKind::Pyannote,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.cohere.com/v2", &en, None),
        AdapterKind::Cohere,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages(
            "https://transcribe.us-east-1.amazonaws.com",
            &en,
            None,
        ),
        AdapterKind::AwsTranscribe,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages(
            "https://example.cognitiveservices.azure.com",
            &en,
            None,
        ),
        AdapterKind::AzureSpeech,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://speech.googleapis.com/v1", &en, None,),
        AdapterKind::GoogleCloud,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.groq.com/openai/v1", &en, None,),
        AdapterKind::Groq,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.rev.ai/speechtotext/v1", &en, None,),
        AdapterKind::RevAi,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://eu1.asr.api.speechmatics.com/v2", &en, None,),
        AdapterKind::Speechmatics,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.together.xyz/v1", &en, None,),
        AdapterKind::Together,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("https://api.x.ai/v1", &en, None),
        AdapterKind::Xai,
    );
    assert_eq!(
        AdapterKind::from_url_and_languages("http://localhost:50060/v1", &en, None),
        AdapterKind::Argmax,
    );
}

#[test]
fn test_append_provider_param_replaces_existing() {
    let url = append_provider_param("https://api.anarlog.so/stt?provider=deepgram", "anarlog");
    assert!(
        url.contains("provider=anarlog"),
        "new provider value should be present: {url}"
    );
    assert!(
        !url.contains("provider=deepgram"),
        "old provider value should be removed: {url}"
    );
    assert_eq!(
        url.matches("provider=").count(),
        1,
        "exactly one provider param expected: {url}"
    );
}

#[test]
fn test_append_provider_param_preserves_other_params() {
    let url = append_provider_param(
        "https://api.anarlog.so/stt?model=cloud&provider=soniox&language=en",
        "anarlog",
    );
    assert!(
        url.contains("model=cloud"),
        "model should be preserved: {url}"
    );
    assert!(
        url.contains("language=en"),
        "language should be preserved: {url}"
    );
    assert!(url.contains("provider=anarlog"));
    assert!(!url.contains("provider=soniox"));
}

#[test]
fn test_append_provider_param_no_existing_provider() {
    let url = append_provider_param("https://api.anarlog.so/stt", "anarlog");
    assert!(url.contains("provider=anarlog"));
    assert_eq!(url.matches("provider=").count(), 1);
}

#[test]
fn openai_diarize_batch_limit_stays_under_api_duration_cap() {
    let diarize = AdapterKind::OpenAI
        .batch_upload_limit(Some("gpt-4o-transcribe-diarize"))
        .expect("openai has a batch upload limit");
    let transcribe = AdapterKind::OpenAI
        .batch_upload_limit(Some("gpt-4o-transcribe"))
        .expect("openai has a batch upload limit");

    assert_eq!(diarize.max_duration, Duration::from_secs(1390));
    assert!(diarize.max_duration < Duration::from_secs(1400));
    assert_eq!(transcribe.max_duration, Duration::from_secs(25 * 60));
    assert_eq!(diarize.max_bytes, transcribe.max_bytes);
}

#[test]
fn openai_compatible_providers_keep_shared_duration_cap() {
    for kind in [AdapterKind::Groq, AdapterKind::Together, AdapterKind::Xai] {
        let limit = kind
            .batch_upload_limit(None)
            .expect("openai-compatible providers have a batch upload limit");
        assert_eq!(limit.max_duration, Duration::from_secs(25 * 60));
    }
}
