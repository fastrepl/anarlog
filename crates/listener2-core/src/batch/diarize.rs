//! On-device speaker diarization for batch transcripts produced by local
//! providers, which return words without speaker labels. Cloud providers
//! diarize server-side, so this only runs when a channel came back unlabeled.

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Instant;

use anlg_pyannote_local::{
    AudioSource, DiarizationConfig, DiarizeRequest, Diarizer, SpeakerBounds, assign_words,
};
use owhisper_interface::batch;

use super::simple::resample_audio_to_channel_files_until;
use super::{BatchParams, BatchProvider, BatchRunOutput};
use crate::BatchRuntime;

pub(super) const LOCAL_DIARIZATION_PROVIDER: &str = "pyannote-local";

/// A participant voiceprint the caller already trusts, forwarded to the
/// diarizer so fragmented clusters of a known voice can be merged.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct KnownSpeaker {
    pub id: String,
    pub embedding: Vec<f32>,
}

pub(super) fn is_local_batch(params: &BatchParams) -> bool {
    match params.provider {
        BatchProvider::WhisperLocal | BatchProvider::Soniqo | BatchProvider::AppleSpeech => true,
        _ => is_loopback_url(&params.base_url),
    }
}

fn is_loopback_url(url: &str) -> bool {
    let authority = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = authority.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    };
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "0.0.0.0")
}

/// Speaker bounds for one recorded channel. Mirrors the live path: on a
/// stereo capture channel 0 is the direct mic (the user alone) and channel 1
/// carries every remote participant, so participant counts shift by one.
/// `None` means the channel should not be diarized.
pub(super) fn channel_speaker_bounds(
    listen_params: &owhisper_interface::ListenParams,
    channel_count: usize,
    channel_index: usize,
) -> Option<SpeakerBounds> {
    let shift = match (channel_count, channel_index) {
        (1, 0) => 0,
        (2, 1) => 1,
        _ => return None,
    };
    let adjust = |value: Option<u32>| -> Option<usize> {
        value.map(|value| {
            usize::try_from(value)
                .unwrap_or(usize::MAX)
                .saturating_sub(shift)
        })
    };
    let num_speakers = adjust(listen_params.num_speakers);
    if num_speakers.is_some_and(|count| count <= 1) {
        return None;
    }
    let max_speakers = adjust(listen_params.max_speakers);
    if max_speakers.is_some_and(|count| count <= 1) {
        return None;
    }
    Some(SpeakerBounds {
        num_speakers,
        min_speakers: adjust(listen_params.min_speakers).filter(|count| *count > 1),
        max_speakers,
    })
}

pub(super) fn channel_needs_diarization(channel: &batch::Channel) -> bool {
    let mut words = channel
        .alternatives
        .iter()
        .flat_map(|alternative| alternative.words.iter())
        .peekable();
    words.peek().is_some() && words.all(|word| word.speaker.is_none())
}

pub(super) async fn apply_local_diarization(
    runtime: Arc<dyn BatchRuntime>,
    params: &BatchParams,
    listen_params: &owhisper_interface::ListenParams,
    output: &mut BatchRunOutput,
) {
    if !is_local_batch(params) {
        return;
    }
    let channel_count = output.response.results.channels.len();
    let targets: Vec<(usize, SpeakerBounds)> = output
        .response
        .results
        .channels
        .iter()
        .enumerate()
        .filter(|(_, channel)| channel_needs_diarization(channel))
        .filter_map(|(index, _)| {
            channel_speaker_bounds(listen_params, channel_count, index)
                .map(|bounds| (index, bounds))
        })
        .collect();
    if targets.is_empty() {
        return;
    }

    let file_path = params.file_path.clone();
    let known_speakers: Vec<anlg_pyannote_local::KnownSpeaker> = params
        .known_speakers
        .iter()
        .map(|speaker| anlg_pyannote_local::KnownSpeaker {
            id: speaker.id.clone(),
            embedding: speaker.embedding.clone(),
        })
        .collect();
    let mut response = std::mem::replace(
        &mut output.response,
        batch::Response {
            metadata: serde_json::Value::Null,
            results: batch::Results { channels: vec![] },
        },
    );
    let started_at = Instant::now();
    let result = tokio::task::spawn_blocking({
        let runtime = runtime.clone();
        move || {
            let outcome = diarize_channels(
                runtime.as_ref(),
                &file_path,
                &targets,
                &known_speakers,
                &mut response,
            );
            (response, outcome)
        }
    })
    .await;

    match result {
        Ok((response, Ok(summary))) => {
            output.response = response;
            tracing::info!(
                anarlog.stt.provider.name = LOCAL_DIARIZATION_PROVIDER,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                diarization.channels = summary.channel_speakers.len(),
                diarization.speakers = ?summary.channel_speakers,
                diarization.identified = summary.identified,
                "local_diarization_completed"
            );
        }
        Ok((response, Err(error))) => {
            output.response = response;
            tracing::warn!(
                anarlog.stt.provider.name = LOCAL_DIARIZATION_PROVIDER,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error = %error,
                "local_diarization_failed"
            );
        }
        Err(error) => {
            tracing::error!(error = %error, "local_diarization_task_join_failed");
        }
    }
}

#[derive(Debug, Default)]
struct DiarizationSummary {
    channel_speakers: Vec<(usize, usize)>,
    identified: usize,
}

fn diarize_channels(
    runtime: &dyn BatchRuntime,
    file_path: &str,
    targets: &[(usize, SpeakerBounds)],
    known_speakers: &[anlg_pyannote_local::KnownSpeaker],
    response: &mut batch::Response,
) -> Result<DiarizationSummary, String> {
    let source = anlg_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    let channel_files =
        resample_audio_to_channel_files_until(file_path, source, || runtime.is_cancelled())?;
    let mut diarizer = Diarizer::new(DiarizationConfig::default()).map_err(|e| e.to_string())?;
    let is_cancelled = || runtime.is_cancelled();

    let mut summary = DiarizationSummary::default();
    for (channel_index, bounds) in targets {
        // A stereo file whose channels were identical collapses to one file;
        // the remote channel then has no audio of its own to diarize.
        let Some(channel_file) = channel_files.get(*channel_index) else {
            continue;
        };
        let mut audio = WavAudioSource::open(channel_file.file.path())?;
        let diarization = diarizer
            .diarize(
                &mut audio,
                &DiarizeRequest {
                    bounds: *bounds,
                    known_speakers,
                    is_cancelled: Some(&is_cancelled),
                },
            )
            .map_err(|e| e.to_string())?;
        if diarization.segments.is_empty() {
            continue;
        }

        let channel = &mut response.results.channels[*channel_index];
        for alternative in &mut channel.alternatives {
            let spans: Vec<(f64, f64)> = alternative
                .words
                .iter()
                .map(|word| (word.start, word.end))
                .collect();
            for (word, speaker) in alternative
                .words
                .iter_mut()
                .zip(assign_words(&spans, &diarization.segments))
            {
                word.speaker = speaker;
            }
        }
        summary
            .channel_speakers
            .push((*channel_index, diarization.speaker_count()));
        summary.identified += diarization
            .speakers
            .iter()
            .filter(|speaker| speaker.identity.is_some())
            .count();
    }

    if let Some(metadata) = response.metadata.as_object_mut() {
        metadata.insert(
            "diarization".to_string(),
            serde_json::json!({
                "provider": LOCAL_DIARIZATION_PROVIDER,
                "channel_speakers": summary.channel_speakers,
                "identified": summary.identified,
            }),
        );
    }
    Ok(summary)
}

struct WavAudioSource {
    reader: hound::WavReader<BufReader<File>>,
    len: usize,
}

impl WavAudioSource {
    fn open(path: &std::path::Path) -> Result<Self, String> {
        let reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
        let spec = reader.spec();
        if spec.channels != 1
            || spec.sample_rate != anlg_pyannote_local::SAMPLE_RATE
            || spec.sample_format != hound::SampleFormat::Float
        {
            return Err(format!(
                "channel file must be 16 kHz mono float, got {} Hz x{} {:?}",
                spec.sample_rate, spec.channels, spec.sample_format
            ));
        }
        let len = reader.len() as usize;
        Ok(Self { reader, len })
    }
}

impl AudioSource for WavAudioSource {
    fn sample_count(&mut self) -> Result<usize, anlg_pyannote_local::Error> {
        Ok(self.len)
    }

    fn read(&mut self, start: usize, out: &mut [f32]) -> Result<(), anlg_pyannote_local::Error> {
        out.fill(0.0);
        if start >= self.len {
            return Ok(());
        }
        let start_u32 = u32::try_from(start)
            .map_err(|e| anlg_pyannote_local::Error::AudioRead(e.to_string()))?;
        self.reader
            .seek(start_u32)
            .map_err(|e| anlg_pyannote_local::Error::AudioRead(e.to_string()))?;
        let available = out.len().min(self.len - start);
        for (slot, sample) in out[..available]
            .iter_mut()
            .zip(self.reader.samples::<f32>())
        {
            *slot = sample.map_err(|e| anlg_pyannote_local::Error::AudioRead(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listen_params(
        num: Option<u32>,
        min: Option<u32>,
        max: Option<u32>,
    ) -> owhisper_interface::ListenParams {
        owhisper_interface::ListenParams {
            num_speakers: num,
            min_speakers: min,
            max_speakers: max,
            ..Default::default()
        }
    }

    fn word(speaker: Option<usize>) -> batch::Word {
        batch::Word {
            word: "hi".to_string(),
            start: 0.0,
            end: 0.5,
            confidence: 1.0,
            channel: 0,
            speaker,
            punctuated_word: None,
        }
    }

    fn channel(words: Vec<batch::Word>) -> batch::Channel {
        batch::Channel {
            alternatives: vec![batch::Alternatives {
                transcript: String::new(),
                confidence: 1.0,
                words,
            }],
        }
    }

    #[test]
    fn mono_channel_uses_participant_counts_directly() {
        let bounds = channel_speaker_bounds(&listen_params(Some(3), None, None), 1, 0).unwrap();
        assert_eq!(bounds, SpeakerBounds::exact(3));

        let free = channel_speaker_bounds(&listen_params(None, None, None), 1, 0).unwrap();
        assert_eq!(free, SpeakerBounds::default());
    }

    #[test]
    fn stereo_remote_channel_drops_the_direct_mic_speaker() {
        let params = listen_params(Some(3), None, None);
        assert!(channel_speaker_bounds(&params, 2, 0).is_none());
        assert_eq!(
            channel_speaker_bounds(&params, 2, 1).unwrap(),
            SpeakerBounds::exact(2)
        );

        // A 1:1 call has a single remote voice: nothing to separate.
        assert!(channel_speaker_bounds(&listen_params(Some(2), None, None), 2, 1).is_none());
        assert!(channel_speaker_bounds(&listen_params(None, None, Some(2)), 2, 1).is_none());

        let range = channel_speaker_bounds(&listen_params(None, Some(2), Some(5)), 2, 1).unwrap();
        assert_eq!(range.min_speakers, None);
        assert_eq!(range.max_speakers, Some(4));
    }

    #[test]
    fn single_speaker_and_extra_channels_are_skipped() {
        assert!(channel_speaker_bounds(&listen_params(Some(1), None, None), 1, 0).is_none());
        assert!(channel_speaker_bounds(&listen_params(None, None, None), 3, 2).is_none());
    }

    #[test]
    fn only_unlabeled_channels_need_diarization() {
        assert!(channel_needs_diarization(&channel(vec![
            word(None),
            word(None)
        ])));
        assert!(!channel_needs_diarization(&channel(vec![
            word(None),
            word(Some(1))
        ])));
        assert!(!channel_needs_diarization(&channel(vec![])));
    }

    fn params(provider: BatchProvider, base_url: &str, file_path: &str) -> BatchParams {
        BatchParams {
            session_id: "s".to_string(),
            provider,
            file_path: file_path.to_string(),
            model: None,
            base_url: base_url.to_string(),
            api_key: String::new(),
            languages: vec![],
            keywords: vec![],
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
            known_speakers: vec![],
        }
    }

    #[test]
    fn local_providers_are_detected() {
        let mut params = params(
            BatchProvider::Deepgram,
            "https://api.deepgram.com/v1",
            "/tmp/a.wav",
        );
        assert!(!is_local_batch(&params));

        params.provider = BatchProvider::WhisperLocal;
        assert!(is_local_batch(&params));

        params.provider = BatchProvider::Am;
        params.base_url = "http://localhost:50060/v1".to_string();
        assert!(is_local_batch(&params));
        params.base_url = "http://127.0.0.1:50060".to_string();
        assert!(is_local_batch(&params));
        params.base_url = "https://api.anarlog.so/stt".to_string();
        assert!(!is_local_batch(&params));
    }

    struct NeverCancelled;

    impl BatchRuntime for NeverCancelled {
        fn emit(&self, _event: crate::BatchEvent) {}

        fn is_cancelled(&self) -> bool {
            false
        }
    }

    fn unlabeled_output(word_count: usize, duration: f64) -> BatchRunOutput {
        let words = (0..word_count)
            .map(|index| {
                let start = index as f64 * duration / word_count as f64;
                batch::Word {
                    start,
                    end: start + duration / word_count as f64,
                    channel: 0,
                    ..word(None)
                }
            })
            .collect();
        BatchRunOutput {
            session_id: "s".to_string(),
            mode: super::super::BatchRunMode::Direct,
            response: batch::Response {
                metadata: serde_json::json!({}),
                results: batch::Results {
                    channels: vec![channel(words)],
                },
            },
        }
    }

    #[tokio::test]
    async fn labels_unlabeled_local_words_from_the_recording() {
        let params = params(
            BatchProvider::WhisperLocal,
            "http://localhost:1234",
            anlg_data::english_1::AUDIO_PATH,
        );
        let listen_params = owhisper_interface::ListenParams::default();
        let mut output = unlabeled_output(200, 100.0);

        apply_local_diarization(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        let words = &output.response.results.channels[0].alternatives[0].words;
        assert!(words.iter().all(|word| word.speaker.is_some()));
        let speakers = words
            .iter()
            .filter_map(|word| word.speaker)
            .collect::<std::collections::HashSet<_>>();
        assert!(
            speakers.len() >= 2,
            "expected two speakers, got {speakers:?}"
        );
        assert_eq!(
            output.response.metadata["diarization"]["provider"],
            LOCAL_DIARIZATION_PROVIDER
        );
    }

    #[tokio::test]
    async fn cloud_output_is_left_untouched() {
        let params = params(
            BatchProvider::Deepgram,
            "https://api.deepgram.com/v1",
            anlg_data::english_1::AUDIO_PATH,
        );
        let mut output = unlabeled_output(10, 5.0);
        let before = output.response.clone();

        apply_local_diarization(
            Arc::new(NeverCancelled),
            &params,
            &owhisper_interface::ListenParams::default(),
            &mut output,
        )
        .await;

        assert_eq!(output.response, before);
    }
}
