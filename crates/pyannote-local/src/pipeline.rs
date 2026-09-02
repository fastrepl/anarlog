//! Offline speaker diarization: sliding-window segmentation, masked speaker
//! embeddings, agglomerative clustering, then reconstruction of who spoke
//! when. Mirrors the structure of the pyannote.audio 3.1 pipeline, tuned for
//! CPU batch use after a recording ends.

use anlg_voiceprint::{
    MIN_UNIQUE_MARGIN, MIN_UNIQUE_SCORE, VoiceprintSpeakerKey, cosine_similarity,
    pick_unique_voiceprint_assignments,
};

use crate::clustering::{self, SpeakerBounds};
use crate::segmentation::{
    FRAME_SIZE, LOCAL_SPEAKERS, SAMPLE_RATE, Segmenter, WINDOW_SAMPLES, WindowActivity,
    frame_start_sample, frames_for_samples,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct DiarizationConfig {
    /// Hop between consecutive 10 s segmentation windows.
    pub step_seconds: f32,
    /// Clean (non-overlapped) speech a local speaker needs inside a window
    /// before its embedding is trusted as a clustering anchor.
    pub min_embedding_seconds: f32,
    /// Speech a local speaker needs inside a window to be embedded at all;
    /// shorter blips are attached to the nearest cluster instead of dropped.
    pub min_activity_seconds: f32,
    /// Centroid-linkage cut on unit-normalised embeddings (euclidean).
    pub clustering_threshold: f32,
    /// Clusters backed by fewer seconds of anchor windows are absorbed by
    /// their neighbours. Expressed in seconds so it stays meaningful when the
    /// step changes (pyannote's 12 anchors at a 1 s step).
    pub min_cluster_seconds: f32,
    /// Drop speaker turns shorter than this after reconstruction.
    pub min_duration_on: f32,
    /// Bridge same-speaker gaps shorter than this after reconstruction.
    pub min_duration_off: f32,
    /// Grow the step on long recordings so compute stays bounded.
    pub max_windows: usize,
    /// Cosine similarity a cluster centroid needs against a known voiceprint
    /// before that voiceprint is allowed to steer clustering.
    pub known_speaker_min_score: f32,
}

impl Default for DiarizationConfig {
    fn default() -> Self {
        Self {
            // 2 s keeps every frame under ~5 overlapping windows so majority
            // voting suppresses per-window false speech; larger steps trade
            // accuracy for speed roughly linearly (see eval/PROGRAM.md).
            step_seconds: 2.0,
            min_embedding_seconds: 1.0,
            min_activity_seconds: 0.3,
            clustering_threshold: 0.7045,
            min_cluster_seconds: 12.0,
            min_duration_on: 0.0,
            min_duration_off: 0.0,
            max_windows: 1800,
            known_speaker_min_score: MIN_UNIQUE_SCORE,
        }
    }
}

/// A voiceprint the caller already trusts (for example a meeting participant
/// with confirmed exemplars). Used to merge fragmented clusters of the same
/// voice and to name the resulting speaker.
#[derive(Debug, Clone, PartialEq)]
pub struct KnownSpeaker {
    pub id: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerSegment {
    pub start: f64,
    pub end: f64,
    pub speaker: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerIdentity {
    pub id: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiarizedSpeaker {
    pub index: usize,
    pub centroid: Vec<f32>,
    pub identity: Option<SpeakerIdentity>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Diarization {
    pub segments: Vec<SpeakerSegment>,
    pub speakers: Vec<DiarizedSpeaker>,
}

impl Diarization {
    pub fn speaker_count(&self) -> usize {
        self.speakers.len()
    }
}

/// Random-access mono 16 kHz audio. Implemented for in-memory buffers; the
/// batch pipeline implements it over a WAV file so long recordings do not
/// need to be resident.
pub trait AudioSource {
    fn sample_count(&mut self) -> Result<usize, crate::Error>;
    /// Fills `out` with samples starting at `start`, zero-padding past the end.
    fn read(&mut self, start: usize, out: &mut [f32]) -> Result<(), crate::Error>;
}

impl AudioSource for &[f32] {
    fn sample_count(&mut self) -> Result<usize, crate::Error> {
        Ok(<[f32]>::len(self))
    }

    fn read(&mut self, start: usize, out: &mut [f32]) -> Result<(), crate::Error> {
        let end = (start + out.len()).min(<[f32]>::len(self));
        let copied = end.saturating_sub(start);
        out[..copied].copy_from_slice(&self[start..end]);
        out[copied..].fill(0.0);
        Ok(())
    }
}

impl AudioSource for Vec<f32> {
    fn sample_count(&mut self) -> Result<usize, crate::Error> {
        Ok(Vec::len(self))
    }

    fn read(&mut self, start: usize, out: &mut [f32]) -> Result<(), crate::Error> {
        self.as_slice().read(start, out)
    }
}

#[derive(Default)]
pub struct DiarizeRequest<'a> {
    pub bounds: SpeakerBounds,
    pub known_speakers: &'a [KnownSpeaker],
    pub is_cancelled: Option<&'a dyn Fn() -> bool>,
    /// Called after each window with the fraction of windows processed, so
    /// callers can keep liveness signals flowing during long recordings.
    pub on_progress: Option<&'a dyn Fn(f32)>,
}

struct WindowEmbedding {
    window: usize,
    local_speaker: usize,
    embedding: Vec<f32>,
    anchor: bool,
}

pub struct Diarizer {
    segmenter: Segmenter,
    embedder: anlg_embedding::EmbeddingExtractor,
    config: DiarizationConfig,
}

impl Diarizer {
    pub fn new(config: DiarizationConfig) -> Result<Self, crate::Error> {
        Ok(Self {
            segmenter: Segmenter::new()?,
            embedder: anlg_embedding::EmbeddingExtractor::new()?,
            config,
        })
    }

    pub fn config(&self) -> &DiarizationConfig {
        &self.config
    }

    pub fn diarize(
        &mut self,
        audio: &mut dyn AudioSource,
        request: &DiarizeRequest<'_>,
    ) -> Result<Diarization, crate::Error> {
        let total = audio.sample_count()?;
        if total == 0 {
            return Ok(Diarization::default());
        }

        let step = self.step_samples(total);
        let window_count = if total <= WINDOW_SAMPLES {
            1
        } else {
            (total - WINDOW_SAMPLES).div_ceil(step) + 1
        };
        let min_anchor_frames = seconds_to_frames(self.config.min_embedding_seconds).max(1);
        let min_activity_frames = seconds_to_frames(self.config.min_activity_seconds).max(1);

        let mut window = vec![0.0f32; WINDOW_SAMPLES];
        let mut mask = vec![0.0f32; WINDOW_SAMPLES];
        let mut activities: Vec<WindowActivity> = Vec::with_capacity(window_count);
        let mut embeddings: Vec<WindowEmbedding> = Vec::new();

        for index in 0..window_count {
            if request.is_cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(crate::Error::Cancelled);
            }
            let start = index * step;
            audio.read(start, &mut window)?;
            let valid = WINDOW_SAMPLES.min(total - start);
            let mut activity = self.segmenter.run_window(&window)?;
            activity
                .frames
                .truncate(frames_for_samples(valid).min(activity.frames.len()));

            for local_speaker in 0..LOCAL_SPEAKERS {
                let active = activity.active_frames(local_speaker);
                if active < min_activity_frames {
                    continue;
                }
                let clean = activity.clean_frames(local_speaker);
                let anchor = clean >= min_anchor_frames;
                fill_sample_mask(&activity, local_speaker, anchor, &mut mask);
                match self.embedder.compute_with_mask_optional(&window, &mask)? {
                    Some(embedding) => embeddings.push(WindowEmbedding {
                        window: index,
                        local_speaker,
                        embedding,
                        anchor,
                    }),
                    None => continue,
                }
            }
            activities.push(activity);
            if let Some(on_progress) = request.on_progress {
                on_progress((index + 1) as f32 / window_count as f32);
            }
        }

        if embeddings.is_empty() {
            return Ok(Diarization::default());
        }

        let (mut labels, mut centroids) = self.cluster(&embeddings, step, request.bounds);
        let identities = self.apply_known_speakers(
            &embeddings,
            &mut labels,
            &mut centroids,
            request.known_speakers,
            request.bounds,
        );

        let speakers: Vec<DiarizedSpeaker> = centroids
            .into_iter()
            .zip(identities)
            .enumerate()
            .map(|(index, (centroid, identity))| DiarizedSpeaker {
                index,
                centroid,
                identity,
            })
            .collect();
        let mut diarization = reconstruct(
            &self.config,
            total,
            step,
            &activities,
            &embeddings,
            &labels,
            speakers,
        );
        renumber_by_first_appearance(&mut diarization);
        Ok(diarization)
    }

    fn step_samples(&self, total: usize) -> usize {
        let requested = seconds_to_frames(self.config.step_seconds).max(1);
        let max_windows = self.config.max_windows.max(2);
        let needed = if total > WINDOW_SAMPLES {
            (total - WINDOW_SAMPLES).div_ceil((max_windows - 1) * FRAME_SIZE)
        } else {
            0
        };
        let frames = requested
            .max(needed)
            .min(frames_for_samples(WINDOW_SAMPLES).max(1));
        frames * FRAME_SIZE
    }

    fn min_cluster_size(&self, step: usize) -> usize {
        let step_seconds = step as f32 / SAMPLE_RATE as f32;
        (self.config.min_cluster_seconds / step_seconds.max(f32::EPSILON)).ceil() as usize
    }

    fn cluster(
        &self,
        embeddings: &[WindowEmbedding],
        step: usize,
        bounds: SpeakerBounds,
    ) -> (Vec<usize>, Vec<Vec<f32>>) {
        let anchors: Vec<usize> = embeddings
            .iter()
            .enumerate()
            .filter(|(_, item)| item.anchor)
            .map(|(index, _)| index)
            .collect();
        let anchors = if anchors.is_empty() {
            (0..embeddings.len()).collect()
        } else {
            anchors
        };
        let anchor_vectors: Vec<Vec<f32>> = anchors
            .iter()
            .map(|index| embeddings[*index].embedding.clone())
            .collect();
        let anchor_labels = clustering::cluster(
            &anchor_vectors,
            self.config.clustering_threshold,
            self.min_cluster_size(step),
            bounds,
        );
        let centroids = clustering::centroids(&anchor_vectors, &anchor_labels);

        let mut labels = vec![usize::MAX; embeddings.len()];
        for (anchor, label) in anchors.iter().zip(&anchor_labels) {
            labels[*anchor] = *label;
        }
        for (index, item) in embeddings.iter().enumerate() {
            if labels[index] == usize::MAX {
                labels[index] =
                    clustering::nearest_centroid(&item.embedding, &centroids).unwrap_or(0);
            }
        }
        (labels, centroids)
    }

    /// Known voiceprints steer the result in two ways: clusters whose best
    /// match is the same known voice are merged (fragmentation of one voice
    /// is the most common local-diarization failure), and the survivors are
    /// named when the match is unique on both sides.
    fn apply_known_speakers(
        &self,
        embeddings: &[WindowEmbedding],
        labels: &mut [usize],
        centroids: &mut Vec<Vec<f32>>,
        known: &[KnownSpeaker],
        bounds: SpeakerBounds,
    ) -> Vec<Option<SpeakerIdentity>> {
        if known.is_empty() || centroids.is_empty() {
            return vec![None; centroids.len()];
        }

        // Several exemplars may describe one person; score against the best.
        let mut ids: Vec<&str> = Vec::new();
        for speaker in known {
            if !ids.contains(&speaker.id.as_str()) {
                ids.push(speaker.id.as_str());
            }
        }
        let score = |centroid: &[f32], id: &str| -> Option<f32> {
            known
                .iter()
                .filter(|speaker| speaker.id == id)
                .filter_map(|speaker| cosine_similarity(centroid, &speaker.embedding))
                .max_by(f32::total_cmp)
        };

        let best_known: Vec<Option<(usize, f32)>> = centroids
            .iter()
            .map(|centroid| {
                ids.iter()
                    .enumerate()
                    .filter_map(|(index, id)| score(centroid, id).map(|score| (index, score)))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .filter(|(_, score)| *score >= self.config.known_speaker_min_score)
            })
            .collect();

        let mut remap: Vec<usize> = (0..centroids.len()).collect();
        let mut cluster_count = centroids.len();
        let floor = bounds
            .num_speakers
            .or(bounds.min_speakers)
            .unwrap_or(1)
            .max(1);
        for known_index in 0..ids.len() {
            let matching: Vec<usize> = (0..centroids.len())
                .filter(|cluster| best_known[*cluster].is_some_and(|(k, _)| k == known_index))
                .collect();
            for cluster in matching.iter().skip(1) {
                if cluster_count <= floor {
                    break;
                }
                remap[*cluster] = matching[0];
                cluster_count -= 1;
            }
        }
        if cluster_count != centroids.len() {
            let mut compact = vec![usize::MAX; centroids.len()];
            let mut next = 0;
            for label in labels.iter_mut() {
                let target = remap[*label];
                if compact[target] == usize::MAX {
                    compact[target] = next;
                    next += 1;
                }
                *label = compact[target];
            }
            let vectors: Vec<Vec<f32>> = embeddings
                .iter()
                .map(|item| item.embedding.clone())
                .collect();
            *centroids = clustering::centroids(&vectors, labels);
        }

        let scores: Vec<(VoiceprintSpeakerKey, &str, f32)> = centroids
            .iter()
            .enumerate()
            .flat_map(|(cluster, centroid)| {
                ids.iter().filter_map(move |id| {
                    score(centroid, id).map(|score| {
                        (
                            VoiceprintSpeakerKey {
                                channel: 0,
                                speaker_index: Some(cluster as i64),
                            },
                            *id,
                            score,
                        )
                    })
                })
            })
            .collect();
        let assignments = pick_unique_voiceprint_assignments(
            &scores,
            self.config.known_speaker_min_score.max(MIN_UNIQUE_SCORE),
            MIN_UNIQUE_MARGIN,
        );

        let mut identities = vec![None; centroids.len()];
        for assignment in assignments {
            if let Some(cluster) = assignment.speaker.speaker_index {
                identities[cluster as usize] = Some(SpeakerIdentity {
                    id: assignment.human_id,
                    score: assignment.score,
                });
            }
        }
        identities
    }
}

/// Votes every window's local-speaker activity onto the global frame grid
/// under its cluster label and turns the majority into speaker turns. Speakers
/// that end up with no turn are dropped.
fn reconstruct(
    config: &DiarizationConfig,
    total: usize,
    step: usize,
    activities: &[WindowActivity],
    embeddings: &[WindowEmbedding],
    labels: &[usize],
    speakers: Vec<DiarizedSpeaker>,
) -> Diarization {
    let speaker_count = speakers.len();
    let frame_count = frames_for_samples(total.max(WINDOW_SAMPLES));
    let mut votes = vec![vec![0u16; frame_count]; speaker_count];
    let mut coverage = vec![0u16; frame_count];

    for (window, activity) in activities.iter().enumerate() {
        let offset = window * step / FRAME_SIZE;
        for frame in 0..activity.frames.len() {
            if let Some(slot) = coverage.get_mut(offset + frame) {
                *slot += 1;
            }
        }
    }
    for (item, &label) in embeddings.iter().zip(labels) {
        let activity = &activities[item.window];
        let offset = item.window * step / FRAME_SIZE;
        for (frame, active) in activity.frames.iter().enumerate() {
            if active[item.local_speaker]
                && let Some(slot) = votes[label].get_mut(offset + frame)
            {
                *slot += 1;
            }
        }
    }

    let mut segments = Vec::new();
    for (speaker, track) in votes.iter().enumerate() {
        let active: Vec<bool> = track
            .iter()
            .zip(&coverage)
            .map(|(votes, coverage)| *coverage > 0 && u32::from(*votes) * 2 >= u32::from(*coverage))
            .collect();
        segments.extend(
            frames_to_segments(
                &active,
                speaker,
                config.min_duration_on,
                config.min_duration_off,
            )
            .into_iter()
            .map(|mut segment| {
                segment.end = segment.end.min(total as f64 / SAMPLE_RATE as f64);
                segment
            })
            .filter(|segment| segment.end > segment.start),
        );
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.speaker.cmp(&b.speaker)));

    let speakers = speakers
        .into_iter()
        .filter(|speaker| {
            segments
                .iter()
                .any(|segment| segment.speaker == speaker.index)
        })
        .collect();
    Diarization { segments, speakers }
}

fn seconds_to_frames(seconds: f32) -> usize {
    ((seconds.max(0.0) * SAMPLE_RATE as f32) / FRAME_SIZE as f32).round() as usize
}

fn frame_time(frame: usize) -> f64 {
    frame_start_sample(frame) as f64 / SAMPLE_RATE as f64
}

fn fill_sample_mask(activity: &WindowActivity, speaker: usize, clean_only: bool, mask: &mut [f32]) {
    mask.fill(0.0);
    for (frame, active) in activity.frames.iter().enumerate() {
        let keep =
            active[speaker] && (!clean_only || active.iter().filter(|value| **value).count() == 1);
        if !keep {
            continue;
        }
        let start = if frame == 0 {
            0
        } else {
            frame_start_sample(frame)
        };
        let end = frame_start_sample(frame + 1).min(mask.len());
        if start < end {
            mask[start..end].fill(1.0);
        }
    }
}

fn frames_to_segments(
    active: &[bool],
    speaker: usize,
    min_duration_on: f32,
    min_duration_off: f32,
) -> Vec<SpeakerSegment> {
    let mut segments: Vec<SpeakerSegment> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (frame, is_active) in active
        .iter()
        .copied()
        .chain(std::iter::once(false))
        .enumerate()
    {
        match (run_start, is_active) {
            (None, true) => run_start = Some(frame),
            (Some(start), false) => {
                let segment = SpeakerSegment {
                    start: frame_time(start),
                    end: frame_time(frame),
                    speaker,
                };
                match segments.last_mut() {
                    Some(previous)
                        if segment.start - previous.end < f64::from(min_duration_off) =>
                    {
                        previous.end = segment.end;
                    }
                    _ => segments.push(segment),
                }
                run_start = None;
            }
            _ => {}
        }
    }
    segments.retain(|segment| segment.end - segment.start >= f64::from(min_duration_on));
    segments
}

fn renumber_by_first_appearance(diarization: &mut Diarization) {
    let mut order: Vec<usize> = Vec::new();
    for segment in &diarization.segments {
        if !order.contains(&segment.speaker) {
            order.push(segment.speaker);
        }
    }
    let position = |speaker: usize| order.iter().position(|s| *s == speaker).unwrap_or(0);
    for segment in &mut diarization.segments {
        segment.speaker = position(segment.speaker);
    }
    for speaker in &mut diarization.speakers {
        speaker.index = position(speaker.index);
    }
    diarization.speakers.sort_by_key(|speaker| speaker.index);
}

/// Speaker for each `(start, end)` word: the speaker with the largest overlap,
/// falling back to the nearest turn when the word lies in a gap. `None` only
/// when there are no segments at all.
pub fn assign_words(words: &[(f64, f64)], segments: &[SpeakerSegment]) -> Vec<Option<usize>> {
    words
        .iter()
        .map(|&(start, end)| {
            if segments.is_empty() {
                return None;
            }
            let mut best_overlap: Option<(f64, usize)> = None;
            let mut nearest: Option<(f64, usize)> = None;
            for segment in segments {
                let overlap = segment.end.min(end) - segment.start.max(start);
                if overlap > 0.0 {
                    if best_overlap.is_none_or(|(best, _)| overlap > best) {
                        best_overlap = Some((overlap, segment.speaker));
                    }
                } else {
                    let distance = if segment.end <= start {
                        start - segment.end
                    } else {
                        segment.start - end
                    };
                    if nearest.is_none_or(|(best, _)| distance < best) {
                        nearest = Some((distance, segment.speaker));
                    }
                }
            }
            best_overlap.or(nearest).map(|(_, speaker)| speaker)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(start: f64, end: f64, speaker: usize) -> SpeakerSegment {
        SpeakerSegment {
            start,
            end,
            speaker,
        }
    }

    #[test]
    fn assigns_words_by_overlap_then_proximity() {
        let segments = vec![segment(0.0, 2.0, 0), segment(2.5, 5.0, 1)];
        let words = [(0.2, 0.6), (1.8, 2.7), (2.1, 2.3), (6.0, 6.4)];
        assert_eq!(
            assign_words(&words, &segments),
            vec![Some(0), Some(1), Some(0), Some(1)]
        );
        assert_eq!(assign_words(&words, &[]), vec![None; 4]);
    }

    #[test]
    fn frames_to_segments_bridges_and_filters() {
        let mut active = vec![true; 100];
        active[40..42].fill(false);
        active[70..].fill(false);
        active[95] = true;

        let raw = frames_to_segments(&active, 0, 0.0, 0.0);
        assert_eq!(raw.len(), 3);

        let bridged = frames_to_segments(&active, 0, 0.1, 0.1);
        assert_eq!(bridged.len(), 1);
        assert!((bridged[0].start - frame_time(0)).abs() < 1e-9);
        assert!((bridged[0].end - frame_time(70)).abs() < 1e-9);
    }

    #[test]
    fn renumbering_follows_first_appearance() {
        let mut diarization = Diarization {
            segments: vec![
                segment(5.0, 6.0, 0),
                segment(0.0, 1.0, 2),
                segment(2.0, 3.0, 1),
            ],
            speakers: (0..3)
                .map(|index| DiarizedSpeaker {
                    index,
                    centroid: vec![],
                    identity: None,
                })
                .collect(),
        };
        diarization
            .segments
            .sort_by(|a, b| a.start.total_cmp(&b.start));
        renumber_by_first_appearance(&mut diarization);
        assert_eq!(
            diarization
                .segments
                .iter()
                .map(|s| s.speaker)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn sample_mask_covers_active_frames_only() {
        let frames = vec![
            [true, false, false],
            [true, true, false],
            [false, true, false],
        ];
        let activity = WindowActivity { frames };
        let mut mask = vec![0.0f32; frame_start_sample(3)];

        fill_sample_mask(&activity, 0, false, &mut mask);
        assert_eq!(mask[0], 1.0);
        assert_eq!(mask[frame_start_sample(1)], 1.0);
        assert_eq!(mask[frame_start_sample(2)], 0.0);

        fill_sample_mask(&activity, 0, true, &mut mask);
        assert_eq!(mask[0], 1.0);
        assert_eq!(mask[frame_start_sample(1)], 0.0);
    }

    fn pcm_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect()
    }

    #[test]
    fn diarizes_two_speaker_fixture() {
        let audio = pcm_bytes_to_f32(anlg_data::english_1::AUDIO);
        let mut diarizer = Diarizer::new(DiarizationConfig::default()).unwrap();
        let diarization = diarizer
            .diarize(&mut audio.as_slice(), &DiarizeRequest::default())
            .unwrap();

        assert!(!diarization.segments.is_empty());
        assert!(diarization.speaker_count() >= 1);
        assert!(diarization.speaker_count() <= 3);
        for pair in diarization.segments.windows(2) {
            assert!(pair[0].start <= pair[1].start);
        }
        let duration = audio.len() as f64 / SAMPLE_RATE as f64;
        assert!(
            diarization
                .segments
                .iter()
                .all(|s| s.end <= duration + 1e-6)
        );
    }

    #[test]
    fn exact_speaker_count_is_respected_on_fixture() {
        let audio = pcm_bytes_to_f32(anlg_data::english_1::AUDIO);
        let mut diarizer = Diarizer::new(DiarizationConfig::default()).unwrap();
        let request = DiarizeRequest {
            bounds: SpeakerBounds::exact(2),
            ..DiarizeRequest::default()
        };
        let diarization = diarizer.diarize(&mut audio.as_slice(), &request).unwrap();
        assert_eq!(diarization.speaker_count(), 2);
    }

    #[test]
    fn known_speaker_names_matching_cluster() {
        let audio = pcm_bytes_to_f32(anlg_data::english_1::AUDIO);
        let mut diarizer = Diarizer::new(DiarizationConfig::default()).unwrap();
        let first = diarizer
            .diarize(&mut audio.as_slice(), &DiarizeRequest::default())
            .unwrap();
        // Two exemplars per person, as a real contact would have.
        let known: Vec<KnownSpeaker> = first
            .speakers
            .iter()
            .flat_map(|speaker| {
                [1.0f32, 0.9].map(|scale| KnownSpeaker {
                    id: format!("human-{}", speaker.index),
                    embedding: speaker.centroid.iter().map(|v| v * scale).collect(),
                })
            })
            .collect();

        let request = DiarizeRequest {
            known_speakers: &known,
            ..DiarizeRequest::default()
        };
        let second = diarizer.diarize(&mut audio.as_slice(), &request).unwrap();
        assert_eq!(second.speaker_count(), first.speaker_count());
        for speaker in &second.speakers {
            let identity = speaker
                .identity
                .as_ref()
                .expect("centroid should match itself");
            assert_eq!(identity.id, format!("human-{}", speaker.index));
        }
    }

    #[test]
    fn progress_reaches_one_after_the_last_window() {
        let audio = pcm_bytes_to_f32(anlg_data::english_1::AUDIO);
        let seen = std::cell::RefCell::new(Vec::new());
        let on_progress = |fraction: f32| seen.borrow_mut().push(fraction);
        let mut diarizer = Diarizer::new(DiarizationConfig::default()).unwrap();
        diarizer
            .diarize(
                &mut audio.as_slice(),
                &DiarizeRequest {
                    on_progress: Some(&on_progress),
                    ..DiarizeRequest::default()
                },
            )
            .unwrap();

        let seen = seen.into_inner();
        assert!(seen.len() > 1);
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
        assert!((seen.last().copied().unwrap() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn silence_yields_nothing() {
        let audio = vec![0.0f32; SAMPLE_RATE as usize * 12];
        let mut diarizer = Diarizer::new(DiarizationConfig::default()).unwrap();
        let diarization = diarizer
            .diarize(&mut audio.as_slice(), &DiarizeRequest::default())
            .unwrap();
        assert!(diarization.segments.is_empty());
        assert_eq!(diarization.speaker_count(), 0);
    }
}
