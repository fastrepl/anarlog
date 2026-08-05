use std::future::{Future, pending};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::{Router, extract::State, http::StatusCode};
use owhisper_client::BatchSttAdapter;
use owhisper_interface::batch::Response;
use tokio::sync::Notify;

use anlg_transcribe_core::TARGET_SAMPLE_RATE;

use super::super::BatchParams;
use super::*;

#[derive(Clone, Default)]
struct HangingHttpAdapter;

impl BatchSttAdapter for HangingHttpAdapter {
    fn provider_name(&self) -> &'static str {
        "hanging-http"
    }

    fn is_supported_languages(
        &self,
        _languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        true
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a reqwest_middleware::ClientWithMiddleware,
        api_base: &'a str,
        _api_key: &'a str,
        _params: &'a owhisper_interface::ListenParams,
        _file_path: P,
    ) -> Pin<
        Box<dyn Future<Output = std::result::Result<Response, owhisper_client::Error>> + Send + 'a>,
    > {
        Box::pin(async move {
            client.post(api_base).body("audio").send().await?;
            panic!("hanging provider unexpectedly responded");
        })
    }
}

#[derive(Clone)]
struct HangingProviderState {
    request_started: Arc<Notify>,
    request_cancelled: Arc<Notify>,
}

struct NotifyOnDrop(Arc<Notify>);

impl Drop for NotifyOnDrop {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

async fn hanging_provider(State(state): State<HangingProviderState>) -> StatusCode {
    let _cancelled = NotifyOnDrop(state.request_cancelled.clone());
    state.request_started.notify_one();
    pending::<()>().await;
    StatusCode::OK
}

fn write_test_wav(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    writer.write_sample(0.0f32).unwrap();
    writer.finalize().unwrap();
}

fn write_test_wav_samples(path: &Path, samples: usize) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..samples {
        writer.write_sample(0.0f32).unwrap();
    }
    writer.finalize().unwrap();
}

#[test]
fn direct_timeout_scales_with_audio_duration_and_is_bounded() {
    assert_eq!(
        direct_batch_timeout_for_audio(None),
        DIRECT_BATCH_TIMEOUT_FLOOR
    );
    assert_eq!(
        direct_batch_timeout_for_audio(Some(Duration::from_secs(60 * 60))),
        Duration::from_secs(2 * 60 * 60 + 5 * 60)
    );
    assert_eq!(
        direct_batch_timeout_for_audio(Some(Duration::from_secs(24 * 60 * 60))),
        DIRECT_BATCH_TIMEOUT_CEILING
    );
}

#[tokio::test]
async fn compresses_oversized_legacy_wav_before_anarlog_upload() {
    let source = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    write_test_wav_samples(source.path(), TARGET_SAMPLE_RATE as usize);
    let source_size = std::fs::metadata(source.path()).unwrap().len();
    let max_bytes = source_size / 2;

    let upload = prepare_anarlog_batch_upload(source.path().to_str().unwrap(), max_bytes)
        .await
        .unwrap();

    assert_eq!(upload.path().extension().unwrap(), "mp3");
    assert!(std::fs::metadata(upload.path()).unwrap().len() <= max_bytes);
    assert!(source.path().exists());
}

#[tokio::test]
async fn keeps_anarlog_uploads_that_fit_without_reencoding() {
    let source = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    write_test_wav(source.path());
    let source_path = source.path().to_path_buf();
    let max_bytes = std::fs::metadata(&source_path).unwrap().len();

    let upload = prepare_anarlog_batch_upload(source_path.to_str().unwrap(), max_bytes)
        .await
        .unwrap();

    assert_eq!(upload.path(), source_path);
}

#[tokio::test]
async fn direct_provider_timeout_cancels_non_responding_request() {
    let request_started = Arc::new(Notify::new());
    let request_cancelled = Arc::new(Notify::new());
    let state = HangingProviderState {
        request_started: request_started.clone(),
        request_cancelled: request_cancelled.clone(),
    };
    let app = Router::new().fallback(hanging_provider).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let audio = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    write_test_wav(audio.path());
    let params = BatchParams {
        session_id: "timeout-test".to_string(),
        provider: super::super::BatchProvider::OpenAI,
        file_path: audio.path().to_string_lossy().into_owned(),
        model: Some("whisper-1".to_string()),
        base_url: format!("http://{address}/v1"),
        api_key: "test".to_string(),
        languages: vec![anlg_language::ISO639::En.into()],
        keywords: vec![],
        num_speakers: None,
        min_speakers: None,
        max_speakers: None,
    };
    let request = tokio::spawn(run_direct_batch_with_timeout::<HangingHttpAdapter>(
        "hanging-http",
        params,
        owhisper_interface::ListenParams {
            model: Some("whisper-1".to_string()),
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        },
        Duration::from_secs(5),
    ));

    tokio::time::timeout(Duration::from_secs(6), request_started.notified())
        .await
        .expect("provider did not receive the request");
    let error = request
        .await
        .expect("batch task panicked")
        .expect_err("non-responding provider should time out");

    match error {
        crate::Error::BatchFailed(failure) => {
            assert_eq!(failure.code(), crate::BatchErrorCode::TimedOut);
            assert!(matches!(
                failure,
                crate::BatchFailure::DirectRequestTimedOut { .. }
            ));
        }
        other => panic!("unexpected timeout error: {other:?}"),
    }
    tokio::time::timeout(Duration::from_secs(2), request_cancelled.notified())
        .await
        .expect("provider request was not cancelled");

    server.abort();
}

#[test]
fn parakeet_batch_reads_bounded_contiguous_chunks_from_disk() {
    let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let total_samples = SONIQO_PARAKEET_MAX_CHUNK_SAMPLES * 2 + TARGET_SAMPLE_RATE as usize;
    write_test_wav_samples(file.path(), total_samples);
    let chunks =
        FixedSoniqoFileChunkIterator::new(file, SONIQO_PARAKEET_MAX_CHUNK_SAMPLES).unwrap();
    let mut previous_end = 0usize;
    let mut chunk_count = 0usize;

    for chunk in chunks {
        let chunk = chunk.unwrap();
        assert_eq!(chunk.sample_start, previous_end);
        assert!(chunk.samples.len() <= SONIQO_PARAKEET_MAX_CHUNK_SAMPLES);
        previous_end = chunk.sample_end;
        chunk_count += 1;
    }

    assert_eq!(chunk_count, 3);
    assert_eq!(previous_end, total_samples);
}

#[test]
fn soniqo_chunk_strategy_preserves_each_model_input_contract() {
    for model in [
        anlg_transcribe_soniqo::SoniqoModel::ParakeetStreaming,
        anlg_transcribe_soniqo::SoniqoModel::ParakeetBatch,
    ] {
        assert_eq!(
            soniqo_chunk_strategy(model),
            SoniqoChunkStrategy::Fixed {
                max_samples: SONIQO_PARAKEET_MAX_CHUNK_SAMPLES,
            }
        );
    }

    for model in [
        anlg_transcribe_soniqo::SoniqoModel::Omnilingual,
        anlg_transcribe_soniqo::SoniqoModel::Qwen3Small,
        anlg_transcribe_soniqo::SoniqoModel::Qwen3Large,
    ] {
        assert_eq!(
            soniqo_chunk_strategy(model),
            SoniqoChunkStrategy::SpeechAware
        );
    }
}

#[test]
fn channel_spooling_stops_cooperatively_when_cancelled() {
    let source_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    write_test_wav_samples(source_file.path(), TARGET_SAMPLE_RATE as usize);
    let source = anlg_audio_utils::source_from_path(source_file.path()).unwrap();
    let cancellation_checks = std::cell::Cell::new(0usize);

    let error = resample_audio_to_channel_files_until("source.wav", source, || {
        let current = cancellation_checks.get();
        cancellation_checks.set(current + 1);
        current > 0
    })
    .expect_err("resampling should stop after cancellation");

    assert!(error.contains(LOCAL_BATCH_CANCELLED));
    assert!(cancellation_checks.get() >= 2);
}

#[test]
fn long_exact_speaker_diarization_is_omitted_from_the_plan() {
    assert!(ensure_soniqo_diarization_within_limit(SONIQO_DIARIZATION_MAX_SAMPLES).is_ok());
    let error =
        ensure_soniqo_diarization_within_limit(SONIQO_DIARIZATION_MAX_SAMPLES + 1).unwrap_err();

    assert!(error.contains("10 minutes"));
    assert!(error.contains("without an exact speaker count"));

    let maximum_channel = [SONIQO_DIARIZATION_MAX_SAMPLES];
    assert!(soniqo_diarization_plan_within_limit(
        &maximum_channel,
        Some(2)
    ));
    let long_channel = [SONIQO_DIARIZATION_MAX_SAMPLES + 1];
    assert!(soniqo_diarization_plan_within_limit(&long_channel, None));
    assert!(!soniqo_diarization_plan_within_limit(
        &long_channel,
        Some(2)
    ));
    let long_stereo = [
        SONIQO_DIARIZATION_MAX_SAMPLES + 1,
        SONIQO_DIARIZATION_MAX_SAMPLES + 1,
    ];
    assert!(soniqo_diarization_plan_within_limit(&long_stereo, Some(2)));
    assert!(!soniqo_diarization_plan_within_limit(&long_stereo, Some(3)));
}

#[test]
fn channel_spooling_preserves_channel_order_without_retaining_audio() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&source_path, spec).unwrap();
    for sample in [0.1, 0.9, 0.2, 0.8] {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
    let source = anlg_audio_utils::source_from_path(&source_path).unwrap();

    let channels = resample_audio_to_channel_files(source_path.to_str().unwrap(), source).unwrap();

    assert_eq!(channels.len(), 2);
    assert_eq!(channels[0].sample_count, 2);
    assert_eq!(channels[0].file.path().parent(), Some(directory.path()));
    let read = channels
        .iter()
        .map(|channel| {
            hound::WavReader::open(channel.file.path())
                .unwrap()
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(read, vec![vec![0.1, 0.2], vec![0.9, 0.8]]);
}

#[test]
fn channel_spooling_discards_effectively_identical_duplicate_channel() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&source_path, spec).unwrap();
    for sample in [0.1, 0.1001, 0.2, 0.2001] {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
    let source = anlg_audio_utils::source_from_path(&source_path).unwrap();

    let channels = resample_audio_to_channel_files(source_path.to_str().unwrap(), source).unwrap();

    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].sample_count, 2);
}

#[test]
fn channel_spooling_rejects_unsupported_channel_count_before_reading_audio() {
    let source_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let writer = hound::WavWriter::create(
        source_file.path(),
        hound::WavSpec {
            channels: (MAX_LOCAL_BATCH_CHANNELS + 1) as u16,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    writer.finalize().unwrap();
    let source = anlg_audio_utils::source_from_path(source_file.path()).unwrap();

    let error = match resample_audio_to_channel_files("source.wav", source) {
        Ok(_) => panic!("unsupported channel count should fail"),
        Err(error) => error,
    };

    assert!(error.contains("at most 8 audio channels"));
    assert!(error.contains("declares 9"));
}

#[test]
fn direct_mic_gate_rejects_only_quiet_chunks() {
    assert!(audio_rms(&[0.0007; 100]) < SONIQO_DIRECT_MIC_MIN_RMS);
    assert!(audio_rms(&[0.0009; 100]) >= SONIQO_DIRECT_MIC_MIN_RMS);
}

#[test]
fn parakeet_batch_window_bounds_force_coreml_shape_3000() {
    let minimum_mel_frames = TARGET_SAMPLE_RATE as usize * 20 / 160 + 1;
    let maximum_mel_frames = SONIQO_PARAKEET_MAX_CHUNK_SAMPLES / 160 + 1;

    assert!(minimum_mel_frames > 2000);
    assert!(maximum_mel_frames <= 3000);
}

#[test]
fn soniqo_language_hint_uses_base_language_code() {
    assert_eq!(soniqo_language_hint(Some("de-DE")).as_deref(), Some("de"));
    assert_eq!(soniqo_language_hint(Some("en_US")).as_deref(), Some("en"));
    assert_eq!(soniqo_language_hint(Some(" fr ")).as_deref(), Some("fr"));
    assert_eq!(soniqo_language_hint(Some("")).as_deref(), None);
    assert_eq!(soniqo_language_hint(None).as_deref(), None);
}

#[test]
fn soniqo_diarization_uses_remote_count_for_system_channel() {
    assert_eq!(soniqo_diarization_speaker_count(Some(3), 2, 0), None);
    assert_eq!(soniqo_diarization_speaker_count(Some(3), 2, 1), Some(2));
}

#[test]
fn soniqo_diarization_uses_total_count_for_mono_audio() {
    assert_eq!(soniqo_diarization_speaker_count(Some(2), 1, 0), Some(2));
    assert_eq!(soniqo_diarization_speaker_count(Some(1), 1, 0), None);
    assert_eq!(soniqo_diarization_speaker_count(None, 1, 0), None);
}

#[test]
fn soniqo_progress_starts_after_chunk_planning() {
    assert_eq!(soniqo_batch_progress(0, 10), SONIQO_PROGRESS_PLANNED);
    assert_eq!(soniqo_batch_progress(0, 0), SONIQO_PROGRESS_PLANNED);
}

#[test]
fn soniqo_progress_caps_before_completion() {
    assert!((soniqo_batch_progress(5, 10) - 0.5).abs() < 1e-9);
    assert_eq!(soniqo_batch_progress(10, 10), SONIQO_PROGRESS_MAX);
    assert_eq!(soniqo_batch_progress(11, 10), SONIQO_PROGRESS_MAX);
}

#[test]
fn collect_soniqo_channel_transcripts_keeps_channel_slots() {
    let transcripts = collect_soniqo_channel_transcripts([
        Ok(anlg_transcribe_soniqo::FileTranscript::new(
            "hello".to_string(),
            1.0,
        )),
        Err("native chunk failed".to_string()),
    ])
    .unwrap();

    assert_eq!(transcripts.len(), 2);
    assert_eq!(transcripts[0].text, "hello");
    assert_eq!(transcripts[1].text, "");
}

#[test]
fn collect_soniqo_channel_transcripts_preserves_later_channel_index() {
    let transcripts = collect_soniqo_channel_transcripts([
        Err("first failed".to_string()),
        Ok(anlg_transcribe_soniqo::FileTranscript::new(
            "system audio".to_string(),
            1.0,
        )),
    ])
    .unwrap();

    let response = anlg_transcribe_soniqo::batch_response_from_channels(
        anlg_transcribe_soniqo::SoniqoModel::ParakeetBatch,
        transcripts,
    );
    let alternative = &response.results.channels[1].alternatives[0];

    assert_eq!(response.results.channels.len(), 2);
    assert_eq!(response.results.channels[0].alternatives[0].transcript, "");
    assert_eq!(alternative.transcript, "system audio");
    assert_eq!(alternative.words[0].channel, 1);
}

#[test]
fn collect_soniqo_channel_transcripts_errors_when_all_channels_fail() {
    let error = collect_soniqo_channel_transcripts([
        Err("first failed".to_string()),
        Err("second failed".to_string()),
    ])
    .unwrap_err();

    assert_eq!(error, "Soniqo failed to transcribe all 2 audio channel(s).");
}
