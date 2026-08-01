use owhisper_interface::stream::{Alternatives, Channel, Metadata, ModelInfo, Word};

use super::*;

fn transcript_response_at(
    transcript: &str,
    words: Vec<Word>,
    is_final: bool,
    channel_idx: i32,
    start: f64,
    duration: f64,
) -> StreamResponse {
    StreamResponse::TranscriptResponse {
        start,
        duration,
        is_final,
        speech_final: is_final,
        from_finalize: false,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: transcript.to_string(),
                words,
                confidence: 1.0,
                languages: vec![],
            }],
        },
        metadata: Metadata {
            request_id: "request".to_string(),
            model_info: ModelInfo {
                name: "model".to_string(),
                version: "1".to_string(),
                arch: "test".to_string(),
            },
            model_uuid: "uuid".to_string(),
            extra: None,
        },
        channel_index: vec![channel_idx, 2],
    }
}

fn word(text: &str, start: f64, end: f64) -> Word {
    Word {
        word: text.to_string(),
        start,
        end,
        confidence: 1.0,
        speaker: None,
        punctuated_word: Some(text.to_string()),
        language: None,
    }
}

fn words_from_text(text: &str, start: f64, duration: f64) -> Vec<Word> {
    let parts = text.split_whitespace().collect::<Vec<_>>();
    let count = parts.len();

    parts
        .into_iter()
        .enumerate()
        .map(|(index, part)| {
            let word_start = start + (index as f64 / count as f64) * duration;
            let word_end = start + ((index + 1) as f64 / count as f64) * duration;
            word(part, word_start, word_end)
        })
        .collect()
}

#[test]
fn soniqo_normalizer_retimes_cumulative_partials() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();

    let mut first =
        transcript_response_at("see", vec![word("see", 0.0, 0.25)], false, 0, 0.0, 0.25);
    normalizer.normalize(&mut first);

    let mut second = transcript_response_at(
        "see the need",
        vec![
            word("see", 0.25, 0.33),
            word("the", 0.33, 0.41),
            word("need", 0.41, 0.50),
        ],
        false,
        0,
        0.25,
        0.25,
    );
    normalizer.normalize(&mut second);

    let StreamResponse::TranscriptResponse {
        start,
        duration,
        channel,
        ..
    } = second
    else {
        panic!("expected transcript response");
    };
    let words = &channel.alternatives[0].words;

    assert_eq!(start, 0.0);
    assert_eq!(duration, 0.5);
    assert_eq!(words.len(), 3);
    assert_eq!(words[0].word, "see");
    assert_eq!(words[0].start, 0.0);
    assert_eq!(words[2].end, 0.45);
}

#[test]
fn soniqo_normalizer_trims_sliding_overlap() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();

    let mut first = transcript_response_at(
        "see the need",
        vec![
            word("see", 0.0, 0.20),
            word("the", 0.20, 0.40),
            word("need", 0.40, 0.60),
        ],
        false,
        0,
        0.0,
        0.60,
    );
    normalizer.normalize(&mut first);

    let mut second = transcript_response_at(
        "the need now",
        vec![
            word("the", 0.60, 0.70),
            word("need", 0.70, 0.80),
            word("now", 0.80, 0.90),
        ],
        false,
        0,
        0.60,
        0.30,
    );
    normalizer.normalize(&mut second);

    let StreamResponse::TranscriptResponse { channel, .. } = second else {
        panic!("expected transcript response");
    };
    let alternative = &channel.alternatives[0];

    assert_eq!(alternative.transcript, "now");
    assert_eq!(
        alternative
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>(),
        vec!["now"],
    );
}

#[test]
fn soniqo_normalizer_updates_active_tokens_when_overlap_drains_partial() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();

    let mut first = transcript_response_at(
        "see the need",
        vec![
            word("see", 0.0, 0.20),
            word("the", 0.20, 0.40),
            word("need", 0.40, 0.60),
        ],
        false,
        0,
        0.0,
        0.60,
    );
    normalizer.normalize(&mut first);

    let mut second = transcript_response_at(
        "the need",
        vec![word("the", 0.60, 0.70), word("need", 0.70, 0.80)],
        false,
        0,
        0.60,
        0.20,
    );
    normalizer.normalize(&mut second);

    let StreamResponse::TranscriptResponse { channel, .. } = second else {
        panic!("expected transcript response");
    };
    assert!(channel.alternatives[0].words.is_empty());

    let state = normalizer.channels.get(&0).expect("channel state");
    assert_eq!(state.active_tokens, vec!["the", "need"]);
    assert_eq!(state.active_start_ms, Some(600));
}

#[test]
fn soniqo_prefix_drain_counts_normalized_tokens_not_words() {
    let mut alternative = Alternatives {
        transcript: ", the need now".to_string(),
        words: vec![
            word(",", 0.60, 0.62),
            word("the", 0.62, 0.70),
            word("need", 0.70, 0.80),
            word("now", 0.80, 0.90),
        ],
        confidence: 1.0,
        languages: vec![],
    };
    let mut current_tokens = normalize_tokens_for_overlap(&alternative.words);

    drain_soniqo_prefix(&mut alternative, &mut current_tokens, 2);

    assert_eq!(alternative.transcript, "now");
    assert_eq!(
        alternative
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>(),
        vec!["now"],
    );
    assert_eq!(current_tokens, vec!["now"]);
}

#[test]
fn soniqo_normalizer_drops_repeated_committed_history() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();
    let repeated = "and it tested if you feel";

    let mut first = transcript_response_at(
        repeated,
        words_from_text(repeated, 0.0, 1.0),
        true,
        0,
        0.0,
        1.0,
    );
    normalizer.normalize(&mut first);

    let mut second = transcript_response_at(
        repeated,
        words_from_text(repeated, 10.0, 1.0),
        true,
        0,
        10.0,
        1.0,
    );
    normalizer.normalize(&mut second);

    let StreamResponse::TranscriptResponse { channel, .. } = second else {
        panic!("expected transcript response");
    };
    let alternative = &channel.alternatives[0];

    assert!(alternative.words.is_empty());
    assert_eq!(alternative.transcript, "");
}

#[test]
fn soniqo_normalizer_trims_repeated_committed_prefix_from_later_update() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();
    let repeated = "and it tested if you feel";
    let filler = "centralized online url for";
    let repeated_with_tail = "and it tested if you feel like new material";

    let mut first = transcript_response_at(
        repeated,
        words_from_text(repeated, 0.0, 1.0),
        true,
        0,
        0.0,
        1.0,
    );
    normalizer.normalize(&mut first);

    let mut second =
        transcript_response_at(filler, words_from_text(filler, 2.0, 1.0), true, 0, 2.0, 1.0);
    normalizer.normalize(&mut second);

    let mut third = transcript_response_at(
        repeated_with_tail,
        words_from_text(repeated_with_tail, 10.0, 1.0),
        false,
        0,
        10.0,
        1.0,
    );
    normalizer.normalize(&mut third);

    let StreamResponse::TranscriptResponse { channel, .. } = third else {
        panic!("expected transcript response");
    };
    let alternative = &channel.alternatives[0];

    assert_eq!(alternative.transcript, "like new material");
    assert_eq!(
        alternative
            .words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>(),
        vec!["like", "new", "material"],
    );
}

#[test]
fn soniqo_normalizer_collapses_internal_partial_loop() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();
    let looped = concat!(
        "yeah but but there's super valuable information in there right ",
        "it's just like it's a little bit like extracting it out of this like junior develop ",
        "yeah but but there's super valuable information in there right ",
        "it's just like it's a little bit like extracting it out of this like junior developer's ",
        "kind of like private freak out it's it's a very difficult problem set because ",
        "it's so you know yeah but but there's super valuable information in there right ",
        "it's just like it's a little bit like extracting it out of this like junior developer's ",
        "kind of like private freak out it's it's a very"
    );

    let mut response = transcript_response_at(
        looped,
        words_from_text(looped, 0.0, 10.0),
        false,
        0,
        0.0,
        10.0,
    );
    normalizer.normalize(&mut response);

    let StreamResponse::TranscriptResponse { channel, .. } = response else {
        panic!("expected transcript response");
    };
    let transcript = &channel.alternatives[0].transcript;

    assert_eq!(transcript.matches("yeah but but").count(), 1);
    assert!(transcript.contains("private freak out"));
}

#[test]
fn soniqo_normalizer_keeps_newer_near_adjacent_internal_rewrite() {
    let mut normalizer = SoniqoTranscriptNormalizer::default();
    let looped = concat!(
        "and something an example i think that should give you pause the big signat ",
        "and something an example i think that should give you pause the big signature ",
        "success so far is certainly alpha fold and of course alph ",
        "and something an example i think that should give you pause the big signature ",
        "success so far is certainly alpha fold and of course alph actually isn't about ai",
    );

    let mut response = transcript_response_at(
        looped,
        words_from_text(looped, 11.0, 13.0),
        false,
        0,
        11.0,
        13.0,
    );
    normalizer.normalize(&mut response);

    let StreamResponse::TranscriptResponse {
        start,
        duration,
        channel,
        ..
    } = response
    else {
        panic!("expected transcript response");
    };
    let transcript = &channel.alternatives[0].transcript;

    assert!(start > 11.0);
    assert!(duration < 13.0);
    assert_eq!(transcript.matches("and something an example").count(), 1);
    assert!(!transcript.contains("big signat and something"));
    assert!(transcript.contains("big signature success so far is certainly alpha fold"));
    assert!(transcript.contains("actually isn't about ai"));
}

#[test]
fn soniqo_engine_replaces_cumulative_live_partials() {
    let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);

    let first = transcript_response_at("see", vec![word("see", 0.0, 0.25)], false, 0, 0.0, 0.25);
    engine.process(&first).expect("first update");

    let second = transcript_response_at(
        "see the need",
        vec![
            word("see", 0.25, 0.33),
            word("the", 0.33, 0.41),
            word("need", 0.41, 0.50),
        ],
        false,
        0,
        0.25,
        0.25,
    );
    let update = engine.process(&second).expect("second update");
    let segment_delta = update.segment_delta.expect("segment delta");

    assert_eq!(segment_delta.upserts.len(), 1);
    assert_eq!(segment_delta.upserts[0].text, "see the need");
}

#[test]
fn soniqo_engine_replaces_rewritten_live_partial_snapshots() {
    let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);

    let first_text = "i've come up with that if you";
    let first = transcript_response_at(
        first_text,
        words_from_text(first_text, 0.0, 0.25),
        false,
        0,
        0.0,
        0.25,
    );
    engine.process(&first).expect("first update");

    let second_text = "i come up with that if you're much smarter actually";
    let second = transcript_response_at(
        second_text,
        words_from_text(second_text, 0.25, 0.25),
        false,
        0,
        0.25,
        0.25,
    );
    let update = engine.process(&second).expect("second update");
    let segment_delta = update.segment_delta.expect("segment delta");

    assert_eq!(segment_delta.upserts.len(), 1);
    assert_eq!(segment_delta.upserts[0].text, second_text);
}

#[test]
fn soniqo_engine_does_not_persist_repeated_final_history() {
    let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
    let repeated = "and it tested if you feel";

    let first = transcript_response_at(
        repeated,
        words_from_text(repeated, 0.0, 1.0),
        true,
        0,
        0.0,
        1.0,
    );
    let first_update = engine.process(&first).expect("first update");

    let second = transcript_response_at(
        repeated,
        words_from_text(repeated, 10.0, 1.0),
        true,
        0,
        10.0,
        1.0,
    );
    assert!(engine.process(&second).is_none());

    let flush_update = engine.flush().expect("flush update");
    let final_text = first_update
        .transcript_delta
        .new_words
        .iter()
        .chain(flush_update.transcript_delta.new_words.iter())
        .map(|word| word.text.as_str())
        .collect::<String>();

    assert_eq!(final_text.trim(), repeated);
}

#[test]
fn soniqo_engine_persists_remaining_partial_without_internal_loop() {
    let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
    let looped = concat!(
        "yeah but but there's super valuable information in there right ",
        "it's just like it's a little bit like extracting it out of this like junior develop ",
        "yeah but but there's super valuable information in there right ",
        "it's just like it's a little bit like extracting it out of this like junior developer's ",
        "kind of like private freak out it's it's a very difficult problem set because ",
        "it's so you know yeah but but there's super valuable information in there right ",
        "it's just like it's a little bit like extracting it out of this like junior developer's ",
        "kind of like private freak out it's it's a very"
    );
    let response = transcript_response_at(
        looped,
        words_from_text(looped, 0.0, 10.0),
        false,
        0,
        0.0,
        10.0,
    );

    engine.process(&response).expect("partial update");
    let flush_update = engine.flush().expect("flush update");
    let segment_delta = flush_update.segment_delta.expect("segment delta");
    let final_text = flush_update
        .transcript_delta
        .new_words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<String>();

    assert!(!flush_update.transcript_delta.new_words.is_empty());
    assert!(flush_update.transcript_delta.partials.is_empty());
    assert_eq!(final_text.matches("yeah but but").count(), 1);
    assert!(final_text.contains("private freak out"));
    assert!(!segment_delta.upserts.is_empty());
}

#[test]
fn soniqo_engine_persists_unfinalized_live_tail_on_flush() {
    let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
    let response = transcript_response_at(
        "visible final tail",
        words_from_text("visible final tail", 10.0, 1.0),
        false,
        0,
        10.0,
        1.0,
    );

    engine.process(&response).expect("partial update");
    let flush_update = engine.flush().expect("flush update");
    let final_text = flush_update
        .transcript_delta
        .new_words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<String>();

    assert_eq!(final_text.trim(), "visible final tail");
    assert!(flush_update.transcript_delta.partials.is_empty());
}

#[test]
fn soniqo_engine_persists_model_final_words_on_flush() {
    let mut engine = LiveTranscriptEngine::new("soniqo", &[], None);
    let response = transcript_response_at(
        "hello world",
        words_from_text("hello world", 0.0, 1.0),
        true,
        0,
        0.0,
        1.0,
    );

    let first_update = engine.process(&response).expect("first update");
    let flush_update = engine.flush().expect("flush update");
    let final_text = first_update
        .transcript_delta
        .new_words
        .iter()
        .chain(flush_update.transcript_delta.new_words.iter())
        .map(|word| word.text.as_str())
        .collect::<String>();

    assert_eq!(final_text.trim(), "hello world");
    assert!(flush_update.transcript_delta.partials.is_empty());
}

#[test]
fn apple_speech_engine_does_not_commit_volatile_hypotheses() {
    let mut engine = LiveTranscriptEngine::new("apple-speech", &[], None);
    let partial = transcript_response_at(
        "what do you think",
        words_from_text("what do you think", 0.0, 4.0),
        false,
        1,
        0.0,
        4.0,
    );
    engine.process(&partial).expect("partial update");

    let final_response = transcript_response_at(
        "what do you think about open source",
        words_from_text("what do you think about open source", 4.0, 4.0),
        true,
        1,
        4.0,
        4.0,
    );
    let final_update = engine.process(&final_response).expect("final update");
    let flush_update = engine.flush().expect("flush update");
    let final_text = final_update
        .transcript_delta
        .new_words
        .iter()
        .chain(flush_update.transcript_delta.new_words.iter())
        .map(|word| word.text.as_str())
        .collect::<String>();

    assert_eq!(final_text.trim(), "what do you think about open source");
    assert!(flush_update.transcript_delta.partials.is_empty());
}

#[test]
fn apple_speech_engine_drops_unfinalized_hypothesis_on_flush() {
    let mut engine = LiveTranscriptEngine::new("apple-speech", &[], None);
    let response = transcript_response_at(
        "volatile tail",
        words_from_text("volatile tail", 10.0, 1.0),
        false,
        1,
        10.0,
        1.0,
    );

    engine.process(&response).expect("partial update");
    let flush_update = engine.flush().expect("flush update");

    assert!(flush_update.transcript_delta.new_words.is_empty());
    assert!(flush_update.transcript_delta.partials.is_empty());
}

#[test]
fn clamps_provider_speakers_to_participant_count() {
    let max_speaker_index = max_speaker_index_for_participants(
        &[
            "self".to_string(),
            "remote-a".to_string(),
            "remote-b".to_string(),
        ],
        Some("self"),
    );
    let mut high_word = word("too-high", 0.0, 0.5);
    high_word.speaker = Some(2);
    let mut negative_word = word("negative", 0.5, 1.0);
    negative_word.speaker = Some(-1);
    let mut response = transcript_response_at(
        "too-high negative",
        vec![high_word, negative_word],
        true,
        0,
        0.0,
        1.0,
    );

    clamp_response_speaker_indices(&mut response, max_speaker_index);

    let StreamResponse::TranscriptResponse { channel, .. } = response else {
        panic!("expected transcript response");
    };
    let words = &channel.alternatives[0].words;

    assert_eq!(words[0].speaker, Some(1));
    assert_eq!(words[1].speaker, Some(0));
}

#[test]
fn clamps_single_remote_speaker_to_zero() {
    let max_speaker_index =
        max_speaker_index_for_participants(&["remote".to_string()], Some("self"));
    let mut high_word = word("too-high", 0.0, 0.5);
    high_word.speaker = Some(2);
    let mut response = transcript_response_at("too-high", vec![high_word], true, 1, 0.0, 0.5);

    clamp_response_speaker_indices(&mut response, max_speaker_index);

    let StreamResponse::TranscriptResponse { channel, .. } = response else {
        panic!("expected transcript response");
    };
    assert_eq!(channel.alternatives[0].words[0].speaker, Some(0));
}

#[test]
fn live_transcript_delta_keeps_speaker_index_on_words() {
    let delta = TranscriptDelta {
        new_words: vec![FinalizedWord {
            id: "word-1".to_string(),
            text: "hello".to_string(),
            start_ms: 0,
            end_ms: 100,
            channel: 0,
            state: anlg_transcript::WordState::Final,
            speaker_index: Some(1),
        }],
        replaced_ids: vec!["replaced".to_string()],
        partials: vec![PartialWord {
            text: "world".to_string(),
            start_ms: 100,
            end_ms: 200,
            channel: 1,
            speaker_index: Some(2),
        }],
    };

    let converted: LiveTranscriptDelta = delta.into();
    assert_eq!(converted.new_words[0].speaker_index, Some(1));
    assert_eq!(converted.partials[0].speaker_index, Some(2));
    assert_eq!(converted.replaced_ids, vec!["replaced"]);
}
