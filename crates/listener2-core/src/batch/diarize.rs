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

use super::simple::{ResampledChannelFile, resample_audio_to_channel_files_until};
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

/// Which recorded audio backs one transcript channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChannelAudio {
    /// One resampled channel file.
    File(usize),
    /// All recorded channels averaged: the provider downmixed a stereo
    /// capture into a single transcript channel, so its words come from the
    /// mic and the system audio together.
    Mix,
}

/// Maps a transcript channel onto recorded audio and a speaker budget.
/// Mirrors the live path: on a stereo capture channel 0 is the direct mic
/// (the user alone) and channel 1 carries every remote participant, so
/// participant counts shift by one. Progressive local providers collapse a
/// stereo recording into one transcript channel; that channel is scored on
/// the mix with the full participant count. `None` means skip the channel.
pub(super) fn plan_channel(
    listen_params: &owhisper_interface::ListenParams,
    transcript_channels: usize,
    recording_channels: usize,
    channel_index: usize,
) -> Option<(ChannelAudio, SpeakerBounds)> {
    let (audio, shift) = match (transcript_channels, recording_channels, channel_index) {
        (1, 1, 0) => (ChannelAudio::File(0), 0),
        (1, 2, 0) => (ChannelAudio::Mix, 0),
        (2, 2, 1) => (ChannelAudio::File(1), 1),
        _ => return None,
    };
    channel_speaker_bounds(listen_params, shift).map(|bounds| (audio, bounds))
}

/// Whether any recording layout the resampler can produce would let
/// [`plan_channel`] accept this transcript channel. The declared channel
/// count is an upper bound: a stereo file with identical channels collapses
/// to one. Lets the common stereo case (unlabeled mic channel, labeled or
/// single-voice remote channel) skip the full resample entirely.
pub(super) fn could_plan_channel(
    listen_params: &owhisper_interface::ListenParams,
    transcript_channels: usize,
    channel_index: usize,
) -> bool {
    let declared = usize::from(listen_params.channels);
    let layouts: &[usize] = match declared {
        0 => &[1, 2],
        1 => &[1],
        _ => &[declared, 1],
    };
    layouts.iter().any(|recording_channels| {
        plan_channel(
            listen_params,
            transcript_channels,
            *recording_channels,
            channel_index,
        )
        .is_some()
    })
}

fn channel_speaker_bounds(
    listen_params: &owhisper_interface::ListenParams,
    shift: usize,
) -> Option<SpeakerBounds> {
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
    let transcript_channels = output.response.results.channels.len();
    // Only word timings cross into the blocking task; the transcript itself
    // stays here so a panic in the diarizer cannot lose it. Channels that no
    // recording layout could plan are dropped now, before the recording is
    // resampled.
    let unlabeled: Vec<UnlabeledChannel> = output
        .response
        .results
        .channels
        .iter()
        .enumerate()
        .filter(|(index, channel)| {
            channel_needs_diarization(channel)
                && could_plan_channel(listen_params, transcript_channels, *index)
        })
        .map(|(index, channel)| UnlabeledChannel {
            index,
            alternatives: channel
                .alternatives
                .iter()
                .map(|alternative| {
                    alternative
                        .words
                        .iter()
                        .map(|word| (word.start, word.end))
                        .collect()
                })
                .collect(),
        })
        .collect();
    if unlabeled.is_empty() {
        return;
    }

    let file_path = params.file_path.clone();
    let listen_params = listen_params.clone();
    let known_speakers: Vec<anlg_pyannote_local::KnownSpeaker> = params
        .known_speakers
        .iter()
        .map(|speaker| anlg_pyannote_local::KnownSpeaker {
            id: speaker.id.clone(),
            embedding: speaker.embedding.clone(),
        })
        .collect();
    let started_at = Instant::now();
    let result = tokio::task::spawn_blocking({
        let runtime = runtime.clone();
        move || {
            diarize_channels(
                runtime.as_ref(),
                &file_path,
                &listen_params,
                transcript_channels,
                &unlabeled,
                &known_speakers,
            )
        }
    })
    .await;

    match result {
        Ok(Ok(labels)) => {
            let summary = apply_labels(&mut output.response, &labels);
            tracing::info!(
                anarlog.stt.provider.name = LOCAL_DIARIZATION_PROVIDER,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                diarization.channels = summary.channel_speakers.len(),
                diarization.speakers = ?summary.channel_speakers,
                diarization.identified = summary.identified,
                "local_diarization_completed"
            );
        }
        Ok(Err(error)) => {
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

struct UnlabeledChannel {
    index: usize,
    /// `(start, end)` of every word, per alternative.
    alternatives: Vec<Vec<(f64, f64)>>,
}

struct ChannelLabels {
    index: usize,
    /// Speaker per word, per alternative, aligned with `UnlabeledChannel`.
    alternatives: Vec<Vec<Option<usize>>>,
    speaker_count: usize,
    identified: usize,
}

#[derive(Debug, Default)]
struct DiarizationSummary {
    channel_speakers: Vec<(usize, usize)>,
    identified: usize,
}

fn apply_labels(response: &mut batch::Response, labels: &[ChannelLabels]) -> DiarizationSummary {
    let mut summary = DiarizationSummary::default();
    for channel_labels in labels {
        let Some(channel) = response.results.channels.get_mut(channel_labels.index) else {
            continue;
        };
        for (alternative, speakers) in channel
            .alternatives
            .iter_mut()
            .zip(&channel_labels.alternatives)
        {
            for (word, speaker) in alternative.words.iter_mut().zip(speakers) {
                word.speaker = *speaker;
            }
        }
        summary
            .channel_speakers
            .push((channel_labels.index, channel_labels.speaker_count));
        summary.identified += channel_labels.identified;
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
    summary
}

fn diarize_channels(
    runtime: &dyn BatchRuntime,
    file_path: &str,
    listen_params: &owhisper_interface::ListenParams,
    transcript_channels: usize,
    unlabeled: &[UnlabeledChannel],
    known_speakers: &[anlg_pyannote_local::KnownSpeaker],
) -> Result<Vec<ChannelLabels>, String> {
    let source = anlg_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    // A stereo file whose channels were identical collapses to one file, so
    // the recording layout is only known after resampling.
    let channel_files =
        resample_audio_to_channel_files_until(file_path, source, || runtime.is_cancelled())?;
    let plans: Vec<(&UnlabeledChannel, ChannelAudio, SpeakerBounds)> = unlabeled
        .iter()
        .filter_map(|channel| {
            plan_channel(
                listen_params,
                transcript_channels,
                channel_files.len(),
                channel.index,
            )
            .map(|(audio, bounds)| (channel, audio, bounds))
        })
        .collect();
    if plans.is_empty() {
        return Ok(Vec::new());
    }

    let mut diarizer = Diarizer::new(DiarizationConfig::default()).map_err(|e| e.to_string())?;
    let is_cancelled = || runtime.is_cancelled();

    let mut labels = Vec::new();
    for (channel, audio, bounds) in plans {
        let mut audio = match audio {
            ChannelAudio::File(index) => {
                MixedAudioSource::open(std::slice::from_ref(&channel_files[index]))?
            }
            ChannelAudio::Mix => MixedAudioSource::open(&channel_files)?,
        };
        let diarization = diarizer
            .diarize(
                &mut audio,
                &DiarizeRequest {
                    bounds,
                    known_speakers,
                    is_cancelled: Some(&is_cancelled),
                },
            )
            .map_err(|e| e.to_string())?;
        if diarization.segments.is_empty() {
            continue;
        }

        labels.push(ChannelLabels {
            index: channel.index,
            alternatives: channel
                .alternatives
                .iter()
                .map(|spans| assign_words(spans, &diarization.segments))
                .collect(),
            speaker_count: diarization.speaker_count(),
            identified: diarization
                .speakers
                .iter()
                .filter(|speaker| speaker.identity.is_some())
                .count(),
        });
    }
    Ok(labels)
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

impl WavAudioSource {
    fn read_into(
        &mut self,
        start: usize,
        out: &mut [f32],
    ) -> Result<(), anlg_pyannote_local::Error> {
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

/// One or more resampled channel files read in lockstep and averaged, so a
/// downmixed transcript can be diarized on the same mix the provider heard.
struct MixedAudioSource {
    channels: Vec<WavAudioSource>,
    scratch: Vec<f32>,
}

impl MixedAudioSource {
    fn open(files: &[ResampledChannelFile]) -> Result<Self, String> {
        if files.is_empty() {
            return Err("no channel files to diarize".to_string());
        }
        let channels = files
            .iter()
            .map(|file| WavAudioSource::open(file.file.path()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            channels,
            scratch: Vec::new(),
        })
    }
}

impl AudioSource for MixedAudioSource {
    fn sample_count(&mut self) -> Result<usize, anlg_pyannote_local::Error> {
        Ok(self
            .channels
            .iter()
            .map(|channel| channel.len)
            .max()
            .unwrap_or(0))
    }

    fn read(&mut self, start: usize, out: &mut [f32]) -> Result<(), anlg_pyannote_local::Error> {
        let (first, rest) = self
            .channels
            .split_first_mut()
            .ok_or_else(|| anlg_pyannote_local::Error::AudioRead("no channels".to_string()))?;
        first.read_into(start, out)?;
        if rest.is_empty() {
            return Ok(());
        }
        self.scratch.resize(out.len(), 0.0);
        for channel in rest.iter_mut() {
            channel.read_into(start, &mut self.scratch)?;
            for (mixed, sample) in out.iter_mut().zip(&self.scratch) {
                *mixed += sample;
            }
        }
        let gain = 1.0 / (rest.len() + 1) as f32;
        for sample in out.iter_mut() {
            *sample *= gain;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anlg_transcribe_core::TARGET_SAMPLE_RATE;

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
    fn mono_recording_uses_participant_counts_directly() {
        let (audio, bounds) = plan_channel(&listen_params(Some(3), None, None), 1, 1, 0).unwrap();
        assert_eq!(audio, ChannelAudio::File(0));
        assert_eq!(bounds, SpeakerBounds::exact(3));

        let (_, free) = plan_channel(&listen_params(None, None, None), 1, 1, 0).unwrap();
        assert_eq!(free, SpeakerBounds::default());
    }

    #[test]
    fn stereo_remote_channel_drops_the_direct_mic_speaker() {
        let params = listen_params(Some(3), None, None);
        assert!(plan_channel(&params, 2, 2, 0).is_none());
        assert_eq!(
            plan_channel(&params, 2, 2, 1).unwrap(),
            (ChannelAudio::File(1), SpeakerBounds::exact(2))
        );

        // A 1:1 call has a single remote voice: nothing to separate.
        assert!(plan_channel(&listen_params(Some(2), None, None), 2, 2, 1).is_none());
        assert!(plan_channel(&listen_params(None, None, Some(2)), 2, 2, 1).is_none());

        let (_, range) = plan_channel(&listen_params(None, Some(2), Some(5)), 2, 2, 1).unwrap();
        assert_eq!(range.min_speakers, None);
        assert_eq!(range.max_speakers, Some(4));
    }

    #[test]
    fn downmixed_stereo_transcript_is_scored_on_the_mix() {
        // Whisper/Argmax collapse a stereo capture into one transcript
        // channel; its words come from mic and system audio together, so the
        // direct mic file alone would miss every remote turn.
        let (audio, bounds) = plan_channel(&listen_params(Some(3), None, None), 1, 2, 0).unwrap();
        assert_eq!(audio, ChannelAudio::Mix);
        assert_eq!(bounds, SpeakerBounds::exact(3));

        // A 1:1 call still has two voices in the mix.
        let (audio, bounds) = plan_channel(&listen_params(Some(2), None, None), 1, 2, 0).unwrap();
        assert_eq!(audio, ChannelAudio::Mix);
        assert_eq!(bounds, SpeakerBounds::exact(2));
    }

    #[test]
    fn single_speaker_and_unexpected_layouts_are_skipped() {
        assert!(plan_channel(&listen_params(Some(1), None, None), 1, 1, 0).is_none());
        assert!(plan_channel(&listen_params(None, None, None), 3, 3, 2).is_none());
        // Identical stereo channels collapse to one file; the transcript's
        // remote channel then has no audio of its own.
        assert!(plan_channel(&listen_params(None, None, None), 2, 1, 1).is_none());
    }

    #[test]
    fn stereo_transcripts_skip_resampling_when_nothing_is_plannable() {
        let stereo = owhisper_interface::ListenParams {
            channels: 2,
            ..listen_params(None, None, None)
        };
        // The direct-mic channel of a stereo capture is never diarized, so a
        // Soniqo/Apple Speech batch whose remote channel is already labeled
        // must not pay for a second resample.
        assert!(!could_plan_channel(&stereo, 2, 0));
        assert!(could_plan_channel(&stereo, 2, 1));
        // A 1:1 call: one remote voice, nothing to separate.
        let one_on_one = owhisper_interface::ListenParams {
            channels: 2,
            ..listen_params(Some(2), None, None)
        };
        assert!(!could_plan_channel(&one_on_one, 2, 1));

        // Downmixed transcripts stay eligible whether the stereo file keeps
        // both channels or collapses to one.
        assert!(could_plan_channel(&stereo, 1, 0));
        let mono = owhisper_interface::ListenParams {
            channels: 1,
            ..listen_params(None, None, None)
        };
        assert!(could_plan_channel(&mono, 1, 0));
        assert!(!could_plan_channel(&mono, 2, 1));
        // Unknown channel count stays permissive.
        assert!(could_plan_channel(&listen_params(None, None, None), 1, 0));
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

    #[test]
    fn loopback_hosts_are_detected_with_and_without_ports() {
        for url in [
            "http://127.0.0.1",
            "http://127.0.0.1/",
            "http://127.0.0.1:50060/v1",
            "http://0.0.0.0",
            "http://localhost",
            "http://localhost:8080/v1?x=1",
            "http://[::1]",
            "http://[::1]:8080",
        ] {
            assert!(is_loopback_url(url), "{url}");
        }
        for url in [
            "https://api.anarlog.so/stt",
            "http://10.0.0.1:8080",
            "http://127.0.0.2",
        ] {
            assert!(!is_loopback_url(url), "{url}");
        }
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

    /// Splits the two-speaker `english_1` fixture into a stereo file with one
    /// speaker per channel, the shape of a mic + system-audio capture.
    fn stereo_fixture() -> tempfile::NamedTempFile {
        #[derive(serde::Deserialize)]
        struct Turn {
            start: u64,
            end: u64,
            speaker: String,
        }
        let turns: Vec<Turn> =
            serde_json::from_str(anlg_data::english_1::DIARIZATION_JSON).unwrap();
        let mono: Vec<f32> = anlg_data::english_1::AUDIO
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect();
        let rate = TARGET_SAMPLE_RATE as usize;
        let mut left = vec![0.0f32; mono.len()];
        let mut right = vec![0.0f32; mono.len()];
        for turn in turns {
            let start = (turn.start as usize * rate / 1000).min(mono.len());
            let end = (turn.end as usize * rate / 1000).min(mono.len());
            let target = if turn.speaker == "speaker0" {
                &mut left
            } else {
                &mut right
            };
            target[start..end].copy_from_slice(&mono[start..end]);
        }

        let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let mut writer = hound::WavWriter::create(
            file.path(),
            hound::WavSpec {
                channels: 2,
                sample_rate: TARGET_SAMPLE_RATE,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for (l, r) in left.iter().zip(&right) {
            writer.write_sample(*l).unwrap();
            writer.write_sample(*r).unwrap();
        }
        writer.finalize().unwrap();
        file
    }

    #[tokio::test]
    async fn downmixed_stereo_transcript_hears_both_channels() {
        let file = stereo_fixture();
        let params = params(
            BatchProvider::WhisperLocal,
            "http://localhost:1234",
            file.path().to_str().unwrap(),
        );
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            ..Default::default()
        };
        let mut output = unlabeled_output(200, 100.0);

        apply_local_diarization(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        // Labelling from the mic channel alone would only ever hear speaker0.
        let speakers = output.response.results.channels[0].alternatives[0]
            .words
            .iter()
            .filter_map(|word| word.speaker)
            .collect::<std::collections::HashSet<_>>();
        assert!(
            speakers.len() >= 2,
            "expected both channels' speakers, got {speakers:?}"
        );
    }

    #[tokio::test]
    async fn failed_diarization_keeps_the_transcript() {
        let params = params(
            BatchProvider::WhisperLocal,
            "http://localhost:1234",
            "/nonexistent/recording.wav",
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
