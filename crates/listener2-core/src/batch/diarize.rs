//! On-device speaker diarization for batch transcripts produced by local
//! providers, which return words without speaker labels. Cloud providers
//! diarize server-side, so this only runs when a channel came back unlabeled.

use std::cell::Cell;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anlg_pyannote_local::{
    AudioSource, DiarizationConfig, DiarizeRequest, Diarizer, SpeakerBounds, assign_words,
};
use owhisper_interface::batch;
use owhisper_interface::batch_stream::BatchStreamEvent;

use super::simple::{ResampledChannelFile, resample_audio_to_channel_files_until};
use super::{BatchParams, BatchProvider, BatchRunOutput};
use crate::{BatchEvent, BatchRuntime};

pub(super) const LOCAL_DIARIZATION_PROVIDER: &str = "pyannote-local";
/// Diarization reports progress from here to 1.0, continuing where local
/// transcription providers stop.
pub(super) const DIARIZATION_PROGRESS_START: f64 = 0.95;
/// Word timings in a batch response are seconds; recorded audio is 16 kHz.
pub(super) const SAMPLE_RATE: usize = anlg_pyannote_local::SAMPLE_RATE as usize;
/// A system-audio channel with no more speech than this carried no call:
/// a stray word or two of noise, not a remote participant.
pub(super) const REMOTE_SILENCE_MAX_SECONDS: f64 = 3.0;
/// Word channel for a microphone shared by several people in the room. The
/// renderer names its speakers instead of folding them into "You".
pub(super) const SHARED_MIC_CHANNEL: i32 = 2;
/// Progressive sessions are torn down after 60 s without a streamed event, so
/// the diarizer emits a progress heartbeat at least this often.
pub(super) const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

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
/// the mix with the full participant count. In an in-person meeting the mic
/// is shared by everyone, so channel 0 takes the full count too. `None`
/// means skip the channel.
pub(super) fn plan_channel(
    listen_params: &owhisper_interface::ListenParams,
    transcript_channels: usize,
    recording_channels: usize,
    channel_index: usize,
    in_person: bool,
) -> Option<(ChannelAudio, SpeakerBounds)> {
    let (audio, shift) = channel_audio(
        transcript_channels,
        recording_channels,
        channel_index,
        in_person,
    )?;
    channel_speaker_bounds(listen_params, shift).map(|bounds| (audio, bounds))
}

/// Which recorded audio a transcript channel was heard from, and how many of the
/// session's participants that audio cannot contain (the direct-mic user on a
/// stereo capture). `None` for the direct-mic channel of a call and for
/// layouts the pipeline does not produce.
pub(super) fn channel_audio(
    transcript_channels: usize,
    recording_channels: usize,
    channel_index: usize,
    in_person: bool,
) -> Option<(ChannelAudio, usize)> {
    match (transcript_channels, recording_channels, channel_index) {
        (1, 1, 0) => Some((ChannelAudio::File(0), 0)),
        (1, 2, 0) => Some((ChannelAudio::Mix, 0)),
        (2, 2, 0) if in_person => Some((ChannelAudio::File(0), 0)),
        (2, 2, 1) => Some((ChannelAudio::File(1), 1)),
        _ => None,
    }
}

/// Whether a stereo capture was an in-person meeting: the session expected
/// several participants, yet the system-audio channel stayed silent, so they
/// were all in the room sharing the microphone. Headphones are not a factor;
/// they keep the far end out of the mic, not the people beside it. Unknown
/// participant counts stay closed: a solo memo and a two-person meeting sound
/// the same to the recorder.
pub(super) fn in_person_capture(
    listen_params: &owhisper_interface::ListenParams,
    response: &batch::Response,
) -> bool {
    let channels = &response.results.channels;
    if channels.len() != 2 {
        return false;
    }
    let several_participants = listen_params.num_speakers.is_some_and(|count| count >= 2)
        || listen_params.min_speakers.is_some_and(|count| count >= 2);
    several_participants && channel_speech_seconds(&channels[1]) <= REMOTE_SILENCE_MAX_SECONDS
}

fn channel_speech_seconds(channel: &batch::Channel) -> f64 {
    channel
        .alternatives
        .first()
        .map(|alternative| {
            alternative
                .words
                .iter()
                .map(|word| (word.end - word.start).max(0.0))
                .sum()
        })
        .unwrap_or(0.0)
}

/// Upper bound on distinct speakers a channel should carry, from the session's
/// participant hints. Unlike [`channel_speaker_bounds`] this keeps a cap of one:
/// a provider that split a single remote voice in two still has to be collapsed.
pub(super) fn expected_speaker_cap(
    listen_params: &owhisper_interface::ListenParams,
    shift: usize,
) -> Option<usize> {
    let adjust = |value: Option<u32>| -> Option<usize> {
        value.map(|value| {
            usize::try_from(value)
                .unwrap_or(usize::MAX)
                .saturating_sub(shift)
        })
    };
    let cap = match (
        adjust(listen_params.num_speakers),
        adjust(listen_params.max_speakers),
    ) {
        (Some(num), Some(max)) => num.min(max),
        (Some(num), None) => num,
        (None, Some(max)) => max,
        (None, None) => return None,
    };
    Some(cap.max(1))
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
    in_person: bool,
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
            in_person,
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
    let in_person = in_person_capture(listen_params, &output.response);
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
                && could_plan_channel(listen_params, transcript_channels, *index, in_person)
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

    let session_id = params.session_id.clone();
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
            let heartbeat = Heartbeat::new(runtime.as_ref(), session_id);
            diarize_channels(
                runtime.as_ref(),
                &heartbeat,
                &DiarizeJob {
                    file_path: &file_path,
                    listen_params: &listen_params,
                    transcript_channels,
                    in_person,
                    unlabeled: &unlabeled,
                    known_speakers: &known_speakers,
                },
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
                diarization.shared_mic = summary.shared_mic,
                "local_diarization_completed"
            );
        }
        Ok(Err(_error)) => {
            tracing::warn!(
                anarlog.stt.provider.name = LOCAL_DIARIZATION_PROVIDER,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error.type = "local_diarization_failed",
                "local_diarization_failed"
            );
        }
        Err(_error) => {
            tracing::error!(
                error.type = "local_task_join_failed",
                "local_diarization_task_join_failed"
            );
        }
    }
}

struct UnlabeledChannel {
    index: usize,
    /// `(start, end)` of every word, per alternative.
    alternatives: Vec<Vec<(f64, f64)>>,
}

struct DiarizeJob<'a> {
    file_path: &'a str,
    listen_params: &'a owhisper_interface::ListenParams,
    transcript_channels: usize,
    in_person: bool,
    unlabeled: &'a [UnlabeledChannel],
    known_speakers: &'a [anlg_pyannote_local::KnownSpeaker],
}

struct ChannelLabels {
    index: usize,
    /// Speaker per word, per alternative, aligned with `UnlabeledChannel`.
    alternatives: Vec<Vec<Option<usize>>>,
    speaker_count: usize,
    identified: usize,
    /// The direct mic of an in-person meeting: its words move to the shared
    /// channel when more than one person turned out to be speaking into it.
    shared_mic: bool,
}

#[derive(Debug, Default)]
struct DiarizationSummary {
    channel_speakers: Vec<(usize, usize)>,
    identified: usize,
    shared_mic: bool,
}

fn apply_labels(response: &mut batch::Response, labels: &[ChannelLabels]) -> DiarizationSummary {
    let mut summary = DiarizationSummary::default();
    for channel_labels in labels {
        let Some(channel) = response.results.channels.get_mut(channel_labels.index) else {
            continue;
        };
        let share_mic = channel_labels.shared_mic && channel_labels.speaker_count >= 2;
        for (alternative, speakers) in channel
            .alternatives
            .iter_mut()
            .zip(&channel_labels.alternatives)
        {
            for (word, speaker) in alternative.words.iter_mut().zip(speakers) {
                word.speaker = *speaker;
                if share_mic {
                    word.channel = SHARED_MIC_CHANNEL;
                }
            }
        }
        summary
            .channel_speakers
            .push((channel_labels.index, channel_labels.speaker_count));
        summary.identified += channel_labels.identified;
        summary.shared_mic |= share_mic;
    }

    if let Some(metadata) = response.metadata.as_object_mut() {
        metadata.insert(
            "diarization".to_string(),
            serde_json::json!({
                "provider": LOCAL_DIARIZATION_PROVIDER,
                "channel_speakers": summary.channel_speakers,
                "identified": summary.identified,
                "shared_mic": summary.shared_mic,
            }),
        );
    }
    summary
}

/// Streams `Progress` events while the diarizer works. Progressive local
/// sessions have an idle monitor that aborts the run when nothing has been
/// streamed for a minute; a long recording spends longer than that here,
/// after transcription has already finished, so silence would throw away a
/// completed transcript.
pub(super) struct Heartbeat<'a> {
    runtime: &'a dyn BatchRuntime,
    session_id: String,
    last_sent: Cell<Option<Instant>>,
}

impl<'a> Heartbeat<'a> {
    pub(super) fn new(runtime: &'a dyn BatchRuntime, session_id: String) -> Self {
        Self {
            runtime,
            session_id,
            last_sent: Cell::new(None),
        }
    }

    /// Emits unless a heartbeat went out within [`HEARTBEAT_INTERVAL`].
    pub(super) fn beat(&self, percentage: f64) {
        let due = self
            .last_sent
            .get()
            .is_none_or(|last| last.elapsed() >= HEARTBEAT_INTERVAL);
        if due {
            self.emit(percentage);
        }
    }

    pub(super) fn emit(&self, percentage: f64) {
        self.last_sent.set(Some(Instant::now()));
        self.runtime.emit(BatchEvent::BatchResponseStreamed {
            session_id: self.session_id.clone(),
            event: BatchStreamEvent::Progress {
                percentage: percentage.clamp(DIARIZATION_PROGRESS_START, 1.0),
                partial_text: None,
            },
        });
    }
}

fn diarize_channels(
    runtime: &dyn BatchRuntime,
    heartbeat: &Heartbeat<'_>,
    job: &DiarizeJob<'_>,
) -> Result<Vec<ChannelLabels>, String> {
    let DiarizeJob {
        file_path,
        listen_params,
        transcript_channels,
        in_person,
        unlabeled,
        known_speakers,
    } = *job;
    heartbeat.emit(DIARIZATION_PROGRESS_START);
    let source = anlg_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    // A stereo file whose channels were identical collapses to one file, so
    // the recording layout is only known after resampling.
    let channel_files = resample_audio_to_channel_files_until(file_path, source, || {
        heartbeat.beat(DIARIZATION_PROGRESS_START);
        runtime.is_cancelled()
    })?;
    let plans: Vec<(&UnlabeledChannel, ChannelAudio, SpeakerBounds)> = unlabeled
        .iter()
        .filter_map(|channel| {
            plan_channel(
                listen_params,
                transcript_channels,
                channel_files.len(),
                channel.index,
                in_person,
            )
            .map(|(audio, bounds)| (channel, audio, bounds))
        })
        .collect();
    if plans.is_empty() {
        return Ok(Vec::new());
    }

    // Loading the models can include an accelerator cache rebuild, which is a
    // full recompile of both; give it a fresh idle window rather than whatever
    // the last throttled resample beat left over.
    heartbeat.emit(DIARIZATION_PROGRESS_START);
    let mut diarizer = Diarizer::new(DiarizationConfig::default()).map_err(|e| e.to_string())?;
    let is_cancelled = || runtime.is_cancelled();
    let plan_count = plans.len() as f64;
    let progress_span = 1.0 - DIARIZATION_PROGRESS_START;

    let mut labels = Vec::new();
    for (plan_index, (channel, audio, bounds)) in plans.into_iter().enumerate() {
        let mut audio = MixedAudioSource::for_audio(&channel_files, audio)?;
        let on_progress = |fraction: f32| {
            let done = (plan_index as f64 + f64::from(fraction)) / plan_count;
            heartbeat.beat(DIARIZATION_PROGRESS_START + progress_span * done);
        };
        let diarization = diarizer
            .diarize(
                &mut audio,
                &DiarizeRequest {
                    bounds,
                    known_speakers,
                    is_cancelled: Some(&is_cancelled),
                    on_progress: Some(&on_progress),
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
            shared_mic: in_person && channel.index == 0 && transcript_channels == 2,
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
pub(super) struct MixedAudioSource {
    channels: Vec<WavAudioSource>,
    scratch: Vec<f32>,
}

impl MixedAudioSource {
    pub(super) fn open(files: &[ResampledChannelFile]) -> Result<Self, String> {
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

    pub(super) fn for_audio(
        files: &[ResampledChannelFile],
        audio: ChannelAudio,
    ) -> Result<Self, String> {
        match audio {
            ChannelAudio::File(index) => {
                let file = files
                    .get(index)
                    .ok_or_else(|| format!("recording has no channel {index}"))?;
                Self::open(std::slice::from_ref(file))
            }
            ChannelAudio::Mix => Self::open(files),
        }
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
        let (audio, bounds) =
            plan_channel(&listen_params(Some(3), None, None), 1, 1, 0, false).unwrap();
        assert_eq!(audio, ChannelAudio::File(0));
        assert_eq!(bounds, SpeakerBounds::exact(3));

        let (_, free) = plan_channel(&listen_params(None, None, None), 1, 1, 0, false).unwrap();
        assert_eq!(free, SpeakerBounds::default());
    }

    #[test]
    fn stereo_remote_channel_drops_the_direct_mic_speaker() {
        let params = listen_params(Some(3), None, None);
        assert!(plan_channel(&params, 2, 2, 0, false).is_none());
        assert_eq!(
            plan_channel(&params, 2, 2, 1, false).unwrap(),
            (ChannelAudio::File(1), SpeakerBounds::exact(2))
        );

        // A 1:1 call has a single remote voice: nothing to separate.
        assert!(plan_channel(&listen_params(Some(2), None, None), 2, 2, 1, false).is_none());
        assert!(plan_channel(&listen_params(None, None, Some(2)), 2, 2, 1, false).is_none());

        let (_, range) =
            plan_channel(&listen_params(None, Some(2), Some(5)), 2, 2, 1, false).unwrap();
        assert_eq!(range.min_speakers, None);
        assert_eq!(range.max_speakers, Some(4));
    }

    #[test]
    fn in_person_meeting_opens_the_direct_mic_with_the_full_count() {
        // Everyone was in the room, so the mic carried all three people and
        // the system-audio channel has nothing to plan.
        let params = listen_params(Some(3), None, None);
        assert_eq!(
            plan_channel(&params, 2, 2, 0, true).unwrap(),
            (ChannelAudio::File(0), SpeakerBounds::exact(3))
        );
        // An in-person 1:1 still has two voices on the mic.
        assert_eq!(
            plan_channel(&listen_params(Some(2), None, None), 2, 2, 0, true).unwrap(),
            (ChannelAudio::File(0), SpeakerBounds::exact(2))
        );
        // The remote channel keeps its call-shaped budget either way.
        assert_eq!(
            plan_channel(&params, 2, 2, 1, true).unwrap(),
            (ChannelAudio::File(1), SpeakerBounds::exact(2))
        );
    }

    #[test]
    fn downmixed_stereo_transcript_is_scored_on_the_mix() {
        // Whisper/Argmax collapse a stereo capture into one transcript
        // channel; its words come from mic and system audio together, so the
        // direct mic file alone would miss every remote turn.
        let (audio, bounds) =
            plan_channel(&listen_params(Some(3), None, None), 1, 2, 0, false).unwrap();
        assert_eq!(audio, ChannelAudio::Mix);
        assert_eq!(bounds, SpeakerBounds::exact(3));

        // A 1:1 call still has two voices in the mix.
        let (audio, bounds) =
            plan_channel(&listen_params(Some(2), None, None), 1, 2, 0, false).unwrap();
        assert_eq!(audio, ChannelAudio::Mix);
        assert_eq!(bounds, SpeakerBounds::exact(2));
    }

    #[test]
    fn single_speaker_and_unexpected_layouts_are_skipped() {
        assert!(plan_channel(&listen_params(Some(1), None, None), 1, 1, 0, false).is_none());
        assert!(plan_channel(&listen_params(None, None, None), 3, 3, 2, false).is_none());
        // Identical stereo channels collapse to one file; the transcript's
        // remote channel then has no audio of its own.
        assert!(plan_channel(&listen_params(None, None, None), 2, 1, 1, false).is_none());
    }

    #[test]
    fn stereo_transcripts_skip_resampling_when_nothing_is_plannable() {
        let stereo = owhisper_interface::ListenParams {
            channels: 2,
            ..listen_params(None, None, None)
        };
        // The direct-mic channel of a call is never diarized, so a
        // Soniqo/Apple Speech batch whose remote channel is already labeled
        // must not pay for a second resample.
        assert!(!could_plan_channel(&stereo, 2, 0, false));
        assert!(could_plan_channel(&stereo, 2, 1, false));
        // A 1:1 call: one remote voice, nothing to separate.
        let one_on_one = owhisper_interface::ListenParams {
            channels: 2,
            ..listen_params(Some(2), None, None)
        };
        assert!(!could_plan_channel(&one_on_one, 2, 1, false));
        // In person, the same 1:1 puts both voices on the mic.
        assert!(could_plan_channel(&one_on_one, 2, 0, true));

        // Downmixed transcripts stay eligible whether the stereo file keeps
        // both channels or collapses to one.
        assert!(could_plan_channel(&stereo, 1, 0, false));
        let mono = owhisper_interface::ListenParams {
            channels: 1,
            ..listen_params(None, None, None)
        };
        assert!(could_plan_channel(&mono, 1, 0, false));
        assert!(!could_plan_channel(&mono, 2, 1, false));
        // Unknown channel count stays permissive.
        assert!(could_plan_channel(
            &listen_params(None, None, None),
            1,
            0,
            false
        ));
    }

    fn two_channel_response(remote_words: Vec<batch::Word>) -> batch::Response {
        batch::Response {
            metadata: serde_json::json!({}),
            results: batch::Results {
                channels: vec![channel(vec![word(None)]), channel(remote_words)],
            },
        }
    }

    #[test]
    fn in_person_needs_several_participants_and_a_silent_remote_channel() {
        let silent = two_channel_response(vec![]);
        assert!(in_person_capture(
            &listen_params(Some(2), None, None),
            &silent
        ));
        assert!(in_person_capture(
            &listen_params(None, Some(2), None),
            &silent
        ));
        // A solo memo and a two-person meeting sound the same to the recorder.
        assert!(!in_person_capture(
            &listen_params(None, None, None),
            &silent
        ));
        assert!(!in_person_capture(
            &listen_params(Some(1), None, None),
            &silent
        ));

        // A stray word of noise on system audio is still silence.
        let noise = two_channel_response(vec![batch::Word {
            start: 10.0,
            end: 11.5,
            channel: 1,
            ..word(None)
        }]);
        assert!(in_person_capture(
            &listen_params(Some(2), None, None),
            &noise
        ));

        // Real far-end speech means a call, however many people were invited.
        let call = two_channel_response(vec![batch::Word {
            start: 10.0,
            end: 20.0,
            channel: 1,
            ..word(Some(0))
        }]);
        assert!(!in_person_capture(
            &listen_params(Some(3), None, None),
            &call
        ));

        // Downmixed transcripts have no remote channel to judge by.
        let downmixed = batch::Response {
            metadata: serde_json::json!({}),
            results: batch::Results {
                channels: vec![channel(vec![word(None)])],
            },
        };
        assert!(!in_person_capture(
            &listen_params(Some(3), None, None),
            &downmixed
        ));
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

    use super::super::test_fixtures::{NeverCancelled, in_person_fixture, params, stereo_fixture};

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

    #[derive(Default)]
    struct RecordingRuntime {
        progress: std::sync::Mutex<Vec<f64>>,
    }

    impl BatchRuntime for RecordingRuntime {
        fn emit(&self, event: crate::BatchEvent) {
            if let crate::BatchEvent::BatchResponseStreamed {
                event: BatchStreamEvent::Progress { percentage, .. },
                ..
            } = event
            {
                self.progress.lock().unwrap().push(percentage);
            }
        }
    }

    #[test]
    fn heartbeat_throttles_but_forced_emits_always_go_out() {
        let runtime = RecordingRuntime::default();
        let heartbeat = Heartbeat::new(&runtime, "s".to_string());

        heartbeat.beat(0.96);
        heartbeat.beat(0.97);
        heartbeat.emit(0.98);
        heartbeat.beat(0.99);

        assert_eq!(*runtime.progress.lock().unwrap(), vec![0.96, 0.98]);
    }

    #[tokio::test]
    async fn diarization_streams_progress_so_idle_monitors_stay_quiet() {
        let runtime = Arc::new(RecordingRuntime::default());
        let params = params(
            BatchProvider::WhisperLocal,
            "http://localhost:1234",
            anlg_data::english_1::AUDIO_PATH,
        );
        let mut output = unlabeled_output(200, 100.0);

        apply_local_diarization(
            runtime.clone(),
            &params,
            &owhisper_interface::ListenParams::default(),
            &mut output,
        )
        .await;

        let progress = runtime.progress.lock().unwrap();
        // One forced beat before the resample and one before the models load,
        // so neither phase starts with a partly spent idle window.
        let start_beats = progress
            .iter()
            .filter(|p| **p == DIARIZATION_PROGRESS_START)
            .count();
        assert!(start_beats >= 2, "{progress:?}");
        assert_eq!(progress[0], DIARIZATION_PROGRESS_START);
        assert!(
            progress
                .iter()
                .all(|p| (DIARIZATION_PROGRESS_START..=1.0).contains(p)),
            "{progress:?}"
        );
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

    /// A stereo transcript whose remote channel is empty: every word came
    /// from the mic.
    fn in_person_output(word_count: usize, duration: f64) -> BatchRunOutput {
        let mut output = unlabeled_output(word_count, duration);
        output.response.results.channels.push(channel(vec![]));
        output
    }

    #[tokio::test]
    async fn in_person_meeting_separates_the_people_sharing_the_mic() {
        let file = in_person_fixture();
        let mut params = params(
            BatchProvider::WhisperLocal,
            "http://localhost:1234",
            file.path().to_str().unwrap(),
        );
        params.num_speakers = Some(2);
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(2),
            ..Default::default()
        };
        let mut output = in_person_output(300, 153.0);

        apply_local_diarization(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        let words = &output.response.results.channels[0].alternatives[0].words;
        assert!(words.iter().all(|word| word.speaker.is_some()));
        assert!(
            words.iter().all(|word| word.channel == SHARED_MIC_CHANNEL),
            "mic words should move to the shared channel"
        );
        let speakers = words
            .iter()
            .filter_map(|word| word.speaker)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(speakers.len(), 2, "expected both people, got {speakers:?}");
        assert_eq!(output.response.metadata["diarization"]["shared_mic"], true);
    }

    #[tokio::test]
    async fn call_with_far_end_speech_keeps_the_mic_as_the_user() {
        // Same recording layout, but the remote channel carried a voice, so
        // this was a call and the mic stays the local user's alone.
        let file = in_person_fixture();
        let mut params = params(
            BatchProvider::WhisperLocal,
            "http://localhost:1234",
            file.path().to_str().unwrap(),
        );
        params.num_speakers = Some(2);
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(2),
            ..Default::default()
        };
        let mut output = in_person_output(300, 153.0);
        output.response.results.channels[1] = channel(vec![batch::Word {
            start: 100.0,
            end: 110.0,
            channel: 1,
            ..word(Some(0))
        }]);
        let before = output.response.clone();

        apply_local_diarization(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        assert_eq!(output.response, before);
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
