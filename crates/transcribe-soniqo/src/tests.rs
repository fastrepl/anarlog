use owhisper_interface::stream;

use super::*;
use crate::responses::SYNTHETIC_BATCH_WORD_SECONDS;

#[test]
fn parses_model_ids() {
    assert_eq!(
        "soniqo-parakeet-streaming".parse::<SoniqoModel>().unwrap(),
        SoniqoModel::ParakeetStreaming
    );
    assert_eq!(
        "soniqo-qwen3-small".parse::<SoniqoModel>().unwrap(),
        SoniqoModel::Qwen3Small
    );
    assert_eq!(
        "soniqo-qwen3-large".parse::<SoniqoModel>().unwrap(),
        SoniqoModel::Qwen3Large
    );
}

#[test]
fn all_includes_available_model_variants() {
    assert_eq!(
        SoniqoModel::all(),
        &[
            SoniqoModel::ParakeetStreaming,
            SoniqoModel::ParakeetBatch,
            SoniqoModel::Omnilingual,
        ]
    );
}

#[test]
fn selectable_includes_advertised_models() {
    assert_eq!(
        SoniqoModel::selectable(),
        &[SoniqoModel::ParakeetStreaming, SoniqoModel::ParakeetBatch]
    );
}

#[test]
fn parakeet_models_support_documented_european_languages() {
    let english = "en-US".parse().unwrap();
    let french = "fr".parse().unwrap();

    assert!(SoniqoModel::ParakeetStreaming.supports_language(&english));
    assert!(SoniqoModel::ParakeetBatch.supports_language(&english));
    assert!(SoniqoModel::ParakeetStreaming.supports_language(&french));
    assert!(SoniqoModel::ParakeetBatch.supports_language(&french));
}

#[test]
fn parakeet_models_reject_unsupported_languages() {
    let korean = "ko".parse().unwrap();

    assert!(!SoniqoModel::ParakeetStreaming.supports_language(&korean));
    assert!(!SoniqoModel::ParakeetBatch.supports_language(&korean));
}

#[test]
fn multilingual_models_support_non_english_languages() {
    let french = "fr".parse().unwrap();

    assert!(SoniqoModel::Omnilingual.supports_language(&french));
    assert!(SoniqoModel::Qwen3Small.supports_language(&french));
    assert!(SoniqoModel::Qwen3Large.supports_language(&french));
}

#[test]
fn live_support_is_gated_by_platform() {
    assert_eq!(
        SoniqoModel::ParakeetStreaming.supports_live_on_current_platform(),
        cfg!(all(target_os = "macos", target_arch = "aarch64")),
    );
    assert!(!SoniqoModel::ParakeetBatch.supports_live_on_current_platform());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn qwen3_platform_error_mentions_macos_15() {
    let error = ensure_supported_platform(SoniqoModel::Qwen3Small).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Soniqo Qwen3 0.6B requires macOS 15 or newer."
    );
}

#[test]
fn streaming_model_uses_batch_model_for_file_transcription() {
    assert_eq!(
        SoniqoModel::ParakeetStreaming.batch_model(),
        SoniqoModel::ParakeetBatch
    );
    assert_eq!(
        SoniqoModel::ParakeetBatch.batch_model(),
        SoniqoModel::ParakeetBatch
    );
}

#[test]
fn batch_response_has_deepgram_shape() {
    let response =
        batch_response_from_text(SoniqoModel::ParakeetBatch, "hello world".to_string(), 2.0);

    assert_eq!(
        response.results.channels[0].alternatives[0].transcript,
        "hello world"
    );
    assert_eq!(response.results.channels[0].alternatives[0].words.len(), 2);
    assert_eq!(response.metadata["model_info"]["arch"], "soniqo");
    assert_eq!(response.metadata["duration"], 2.0);
    assert_eq!(response.metadata["timing_source"], "synthetic_text");
}

#[test]
fn batch_response_preserves_channel_indexes() {
    let response = batch_response_from_channels(
        SoniqoModel::Omnilingual,
        vec![
            FileTranscript::new("mic words".to_string(), 2.0),
            FileTranscript::new("speaker words".to_string(), 3.0),
        ],
    );

    assert_eq!(response.metadata["channels"], 2);
    assert_eq!(response.metadata["duration"], 3.0);
    assert_eq!(response.results.channels.len(), 2);
    assert_eq!(
        response.results.channels[0].alternatives[0].transcript,
        "mic words"
    );
    assert_eq!(
        response.results.channels[1].alternatives[0].transcript,
        "speaker words"
    );
    assert_eq!(
        response.results.channels[0].alternatives[0].words[0].channel,
        0
    );
    assert_eq!(
        response.results.channels[1].alternatives[0].words[0].channel,
        1
    );
}

#[test]
fn batch_response_uses_compact_synthetic_word_timing() {
    let response = batch_response_from_text(
        SoniqoModel::ParakeetBatch,
        "one two three four".to_string(),
        120.0,
    );
    let words = &response.results.channels[0].alternatives[0].words;

    assert_eq!(words[0].start, 0.0);
    assert_eq!(words[0].end, SYNTHETIC_BATCH_WORD_SECONDS);
    assert_eq!(words[3].end, 4.0 * SYNTHETIC_BATCH_WORD_SECONDS);
    assert!(words[3].end < 3.0);
    assert_eq!(response.metadata["duration"], 120.0);
}

#[test]
fn batch_response_offsets_synthetic_words_by_chunk_start() {
    let response = batch_response_from_channels(
        SoniqoModel::ParakeetBatch,
        vec![FileTranscript::from_chunks(
            vec![
                FileTranscriptChunk {
                    text: "early words".to_string(),
                    start_seconds: 0.0,
                    duration_seconds: 29.5,
                },
                FileTranscriptChunk {
                    text: "later words".to_string(),
                    start_seconds: 29.5,
                    duration_seconds: 29.5,
                },
            ],
            59.0,
        )],
    );
    let words = &response.results.channels[0].alternatives[0].words;

    assert_eq!(words[0].start, 0.0);
    assert_eq!(words[1].start, SYNTHETIC_BATCH_WORD_SECONDS);
    assert_eq!(words[2].start, 29.5);
    assert_eq!(words[3].start, 29.5 + SYNTHETIC_BATCH_WORD_SECONDS);
    assert_eq!(response.metadata["timing_source"], "synthetic_text");
}

#[test]
fn batch_response_aligns_words_to_diarized_speech() {
    let mut transcript = FileTranscript::from_chunks(
        vec![FileTranscriptChunk {
            text: "lex one two george three four".to_string(),
            start_seconds: 0.0,
            duration_seconds: 10.0,
        }],
        10.0,
    );
    transcript.speaker_segments = vec![
        DiarizationSegment {
            start_seconds: 1.0,
            end_seconds: 4.0,
            speaker_index: 0,
        },
        DiarizationSegment {
            start_seconds: 6.0,
            end_seconds: 9.0,
            speaker_index: 1,
        },
    ];

    let response = batch_response_from_channels(SoniqoModel::ParakeetBatch, vec![transcript]);
    let words = &response.results.channels[0].alternatives[0].words;

    assert_eq!(
        words.iter().map(|word| word.speaker).collect::<Vec<_>>(),
        vec![Some(0), Some(0), Some(0), Some(1), Some(1), Some(1)]
    );
    assert!(words.windows(2).all(|pair| pair[0].start <= pair[1].start));
    assert!(words[0].start >= 1.0);
    assert!(words[3].start >= 6.0);
    assert_eq!(response.metadata["timing_source"], "diarized_speech");
}

#[test]
fn batch_response_normalizes_internal_whitespace() {
    let response = batch_response_from_text(
        SoniqoModel::ParakeetBatch,
        "eins zwei\n drei\tvier".to_string(),
        4.0,
    );
    let alternative = &response.results.channels[0].alternatives[0];

    assert_eq!(alternative.transcript, "eins zwei drei vier");
    assert_eq!(
        alternative
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>(),
        vec!["eins", "zwei", "drei", "vier"]
    );
}

#[test]
fn live_response_keeps_source_channel() {
    let partial = LivePartial {
        source: "system".to_string(),
        text: "hello".to_string(),
        is_final: true,
    };

    let response = partial.into_stream_response(SoniqoModel::ParakeetStreaming, 0.0, 0.5);
    let stream::StreamResponse::TranscriptResponse { channel_index, .. } = response else {
        panic!("expected transcript response");
    };

    assert_eq!(channel_index, vec![1, 2]);
}
