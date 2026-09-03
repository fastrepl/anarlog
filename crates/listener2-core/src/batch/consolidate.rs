//! Post-pass for hosted batch transcripts. Cloud providers label speakers but
//! never see the session's participant count or voiceprints (Soniox and
//! Deepgram have no speaker-count parameter, and biometrics stay on device),
//! so a remote channel often comes back with more "speakers" than people.
//! This embeds each provider speaker's clean speech with the same model the
//! voiceprint store uses, merges labels that are one known voice or that
//! exceed the expected count, and renumbers what is left.

use std::sync::Arc;
use std::time::Instant;

use anlg_pyannote_local::clustering::normalize;
use anlg_voiceprint::{
    MIN_UNIQUE_MARGIN, MIN_UNIQUE_SCORE, SelectedSpan, SpanConfig, SpanWord, cosine_similarity,
    select_speaker_spans,
};
use owhisper_interface::batch;

use super::diarize::{
    ChannelAudio, DIARIZATION_PROGRESS_START, Heartbeat, MixedAudioSource, SAMPLE_RATE,
    channel_audio, expected_speaker_cap, is_local_batch,
};
use super::simple::resample_audio_to_channel_files_until;
use super::{BatchParams, BatchRunOutput, KnownSpeaker};
use crate::BatchRuntime;

pub(super) const CONSOLIDATION_PROVIDER: &str = "wespeaker-local";

/// A provider speaker on one transcript channel, ready to be compared.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SpeakerProfile {
    pub(super) label: usize,
    /// Unit-length mean of the speaker's span embeddings; `None` when the
    /// provider gave it too little clean speech to embed.
    pub(super) centroid: Option<Vec<f32>>,
    /// Total clean speech backing the centroid, so merges weight by evidence.
    pub(super) weight: f32,
    /// Word start times, for folding unembeddable speakers into a neighbour.
    pub(super) word_starts: Vec<f64>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ChannelConsolidation {
    pub(super) index: usize,
    pub(super) before: usize,
    pub(super) after: usize,
    pub(super) identified: usize,
}

pub(super) async fn consolidate_hosted_speakers(
    runtime: Arc<dyn BatchRuntime>,
    params: &BatchParams,
    listen_params: &owhisper_interface::ListenParams,
    output: &mut BatchRunOutput,
) {
    if is_local_batch(params) {
        return;
    }
    let transcript_channels = output.response.results.channels.len();
    let recording_channels = usize::from(listen_params.channels).max(1);
    let has_known = !params.known_speakers.is_empty();

    let work: Vec<ChannelWork> = output
        .response
        .results
        .channels
        .iter()
        .enumerate()
        .filter_map(|(index, channel)| {
            let (audio, shift) = channel_audio(transcript_channels, recording_channels, index)?;
            let labels = distinct_labels(channel);
            if labels.len() < 2 && !has_known {
                return None;
            }
            let cap = expected_speaker_cap(listen_params, shift);
            let over_cap = cap.is_some_and(|cap| labels.len() > cap);
            if !over_cap && !has_known {
                return None;
            }
            Some(ChannelWork {
                index,
                audio,
                cap,
                words: word_timings(channel),
            })
        })
        .collect();
    if work.is_empty() {
        return;
    }

    let session_id = params.session_id.clone();
    let file_path = params.file_path.clone();
    let known = params.known_speakers.clone();
    let started_at = Instant::now();
    let result = tokio::task::spawn_blocking({
        let runtime = runtime.clone();
        move || {
            let heartbeat = Heartbeat::new(runtime.as_ref(), session_id);
            consolidate_channels(runtime.as_ref(), &heartbeat, &file_path, &work, &known)
        }
    })
    .await;

    match result {
        Ok(Ok(relabels)) => {
            let summary = apply_relabels(&mut output.response, &relabels);
            tracing::info!(
                anarlog.stt.provider.name = CONSOLIDATION_PROVIDER,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                consolidation.channels = ?summary,
                "speaker_consolidation_completed"
            );
        }
        Ok(Err(error)) => {
            tracing::warn!(
                anarlog.stt.provider.name = CONSOLIDATION_PROVIDER,
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                error = %error,
                "speaker_consolidation_failed"
            );
        }
        Err(error) => {
            tracing::error!(error = %error, "speaker_consolidation_task_join_failed");
        }
    }
}

struct ChannelWork {
    index: usize,
    audio: ChannelAudio,
    cap: Option<usize>,
    /// `(start, end, speaker)` per word, per alternative.
    words: Vec<Vec<(f64, f64, Option<usize>)>>,
}

struct ChannelRelabel {
    index: usize,
    /// Old provider label → new label.
    mapping: Vec<(usize, usize)>,
    identified: usize,
}

fn distinct_labels(channel: &batch::Channel) -> Vec<usize> {
    let mut labels = Vec::new();
    for word in channel
        .alternatives
        .iter()
        .flat_map(|alternative| alternative.words.iter())
    {
        if let Some(speaker) = word.speaker
            && !labels.contains(&speaker)
        {
            labels.push(speaker);
        }
    }
    labels
}

fn word_timings(channel: &batch::Channel) -> Vec<Vec<(f64, f64, Option<usize>)>> {
    channel
        .alternatives
        .iter()
        .map(|alternative| {
            alternative
                .words
                .iter()
                .map(|word| (word.start, word.end, word.speaker))
                .collect()
        })
        .collect()
}

fn consolidate_channels(
    runtime: &dyn BatchRuntime,
    heartbeat: &Heartbeat<'_>,
    file_path: &str,
    work: &[ChannelWork],
    known: &[KnownSpeaker],
) -> Result<Vec<ChannelRelabel>, String> {
    heartbeat.emit(DIARIZATION_PROGRESS_START);
    let source = anlg_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    let channel_files = resample_audio_to_channel_files_until(file_path, source, || {
        heartbeat.beat(DIARIZATION_PROGRESS_START);
        runtime.is_cancelled()
    })?;

    heartbeat.emit(DIARIZATION_PROGRESS_START);
    let mut extractor = anlg_embedding::EmbeddingExtractor::new().map_err(|e| e.to_string())?;
    let progress_span = 1.0 - DIARIZATION_PROGRESS_START;

    let mut relabels = Vec::new();
    for (work_index, channel) in work.iter().enumerate() {
        if runtime.is_cancelled() {
            return Err("cancelled".to_string());
        }
        let mut audio = MixedAudioSource::for_audio(&channel_files, channel.audio)?;
        let mut profiles = speaker_profiles(&channel.words);
        let spans = select_speaker_spans(&span_words(&channel.words), &SpanConfig::default());
        embed_profiles(&mut profiles, &spans, &mut audio, &mut extractor)?;
        heartbeat.beat(
            DIARIZATION_PROGRESS_START
                + progress_span * (work_index + 1) as f64 / work.len() as f64,
        );

        let plan = consolidate_profiles(profiles, channel.cap, known);
        relabels.push(ChannelRelabel {
            index: channel.index,
            mapping: plan.mapping,
            identified: plan.identified,
        });
    }
    Ok(relabels)
}

fn span_words(words: &[Vec<(f64, f64, Option<usize>)>]) -> Vec<SpanWord> {
    // Spans are selected on the first alternative; the others share its
    // timing closely enough that its clean stretches serve every alternative.
    words
        .first()
        .into_iter()
        .flatten()
        .filter_map(|(start, end, speaker)| {
            speaker.map(|speaker| SpanWord {
                start_ms: (start * 1000.0) as i64,
                end_ms: (end * 1000.0) as i64,
                channel: 0,
                speaker_index: Some(speaker as i64),
            })
        })
        .collect()
}

fn speaker_profiles(words: &[Vec<(f64, f64, Option<usize>)>]) -> Vec<SpeakerProfile> {
    let mut profiles: Vec<SpeakerProfile> = Vec::new();
    for (start, _, speaker) in words.iter().flatten() {
        let Some(label) = speaker else {
            continue;
        };
        match profiles.iter_mut().find(|profile| profile.label == *label) {
            Some(profile) => profile.word_starts.push(*start),
            None => profiles.push(SpeakerProfile {
                label: *label,
                centroid: None,
                weight: 0.0,
                word_starts: vec![*start],
            }),
        }
    }
    profiles
}

fn embed_profiles(
    profiles: &mut [SpeakerProfile],
    spans: &[SelectedSpan],
    audio: &mut MixedAudioSource,
    extractor: &mut anlg_embedding::EmbeddingExtractor,
) -> Result<(), String> {
    let mut sums: Vec<(Vec<f32>, f32)> = vec![(Vec::new(), 0.0); profiles.len()];
    let mut buffer = Vec::new();
    for span in spans {
        let Some(label) = span.speaker_index else {
            continue;
        };
        let Some(slot) = profiles
            .iter()
            .position(|profile| profile.label == label as usize)
        else {
            continue;
        };
        let start = (span.start_ms.max(0) as usize) * SAMPLE_RATE / 1000;
        let end = (span.end_ms.max(0) as usize) * SAMPLE_RATE / 1000;
        if end <= start {
            continue;
        }
        buffer.resize(end - start, 0.0);
        anlg_pyannote_local::AudioSource::read(audio, start, &mut buffer)
            .map_err(|e| e.to_string())?;
        let Some(embedding) = extractor
            .compute_optional(&buffer)
            .map_err(|e| e.to_string())?
        else {
            continue;
        };
        let weight = span.duration_ms() as f32 / 1000.0;
        let (sum, total) = &mut sums[slot];
        if sum.is_empty() {
            sum.resize(embedding.len(), 0.0);
        }
        for (acc, value) in sum.iter_mut().zip(normalize(&embedding)) {
            *acc += value * weight;
        }
        *total += weight;
    }
    for (profile, (sum, total)) in profiles.iter_mut().zip(sums) {
        if total > 0.0 {
            profile.centroid = Some(normalize(&sum));
            profile.weight = total;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub(super) struct ConsolidationPlan {
    /// Old provider label → new label, renumbered by first appearance.
    pub(super) mapping: Vec<(usize, usize)>,
    pub(super) identified: usize,
}

/// Decides which provider labels are the same voice. Known participants come
/// first: two labels that both match one voiceprint are one person. Then the
/// count: while more labels remain than the session can hold, the two most
/// similar are merged. Like the on-device diarizer, a participant count is
/// treated as authoritative rather than as a soft hint. Only then do labels
/// with no clean speech to embed follow whichever embedded speaker they sit
/// closest to in time; within the count a brief talker keeps their own label.
pub(super) fn consolidate_profiles(
    profiles: Vec<SpeakerProfile>,
    cap: Option<usize>,
    known: &[KnownSpeaker],
) -> ConsolidationPlan {
    let original_order: Vec<usize> = profiles.iter().map(|profile| profile.label).collect();
    let mut clusters: Vec<Cluster> = profiles
        .into_iter()
        .map(|profile| Cluster {
            labels: vec![profile.label],
            centroid: profile.centroid,
            weight: profile.weight,
            word_starts: profile.word_starts,
            known_id: None,
        })
        .collect();

    let identified = merge_known_duplicates(&mut clusters, known);
    if let Some(cap) = cap.filter(|cap| clusters.len() > *cap) {
        fold_unembedded(&mut clusters);
        merge_until_within_cap(&mut clusters, cap);
    }

    let mut mapping = Vec::new();
    let mut next = 0;
    for label in original_order {
        if mapping.iter().any(|(old, _)| *old == label) {
            continue;
        }
        let cluster = clusters
            .iter()
            .find(|cluster| cluster.labels.contains(&label))
            .expect("every label belongs to a cluster");
        let assigned = mapping
            .iter()
            .find(|(old, _)| cluster.labels.contains(old))
            .map(|(_, new)| *new)
            .unwrap_or_else(|| {
                next += 1;
                next - 1
            });
        mapping.push((label, assigned));
    }
    ConsolidationPlan {
        mapping,
        identified,
    }
}

#[derive(Debug)]
struct Cluster {
    labels: Vec<usize>,
    centroid: Option<Vec<f32>>,
    weight: f32,
    word_starts: Vec<f64>,
    /// The participant this cluster's voiceprint uniquely matched, if any.
    known_id: Option<String>,
}

impl Cluster {
    fn absorb(&mut self, other: Cluster) {
        self.labels.extend(other.labels);
        self.word_starts.extend(other.word_starts);
        if self.known_id.is_none() {
            self.known_id = other.known_id;
        }
        match (&mut self.centroid, other.centroid) {
            (Some(mine), Some(theirs)) => {
                let total = self.weight + other.weight;
                if total > 0.0 {
                    for (acc, value) in mine.iter_mut().zip(theirs) {
                        *acc = (*acc * self.weight + value * other.weight) / total;
                    }
                    *mine = normalize(mine);
                }
                self.weight = total;
            }
            (None, Some(theirs)) => {
                self.centroid = Some(theirs);
                self.weight = other.weight;
            }
            _ => {}
        }
    }
}

fn best_known<'a>(centroid: &[f32], known: &'a [KnownSpeaker]) -> Option<&'a str> {
    let mut best: Option<(&str, f32)> = None;
    let mut second = f32::NEG_INFINITY;
    let mut ids: Vec<&str> = Vec::new();
    for speaker in known {
        if !ids.contains(&speaker.id.as_str()) {
            ids.push(&speaker.id);
        }
    }
    for id in ids {
        let score = known
            .iter()
            .filter(|speaker| speaker.id == id)
            .filter_map(|speaker| cosine_similarity(centroid, &speaker.embedding))
            .fold(f32::NEG_INFINITY, f32::max);
        match best {
            Some((_, best_score)) if score <= best_score => second = second.max(score),
            Some((_, best_score)) => {
                second = best_score;
                best = Some((id, score));
            }
            None => best = Some((id, score)),
        }
    }
    let (id, score) = best?;
    (score >= MIN_UNIQUE_SCORE && (second.is_infinite() || score - second >= MIN_UNIQUE_MARGIN))
        .then_some(id)
}

fn merge_known_duplicates(clusters: &mut Vec<Cluster>, known: &[KnownSpeaker]) -> usize {
    if known.is_empty() {
        return 0;
    }
    let matches: Vec<Option<String>> = clusters
        .iter()
        .map(|cluster| {
            cluster
                .centroid
                .as_deref()
                .and_then(|centroid| best_known(centroid, known))
                .map(str::to_string)
        })
        .collect();
    let mut merged: Vec<Cluster> = Vec::new();
    for (mut cluster, id) in clusters.drain(..).zip(matches) {
        let existing = id.as_ref().and_then(|id| {
            merged
                .iter()
                .position(|other| other.known_id.as_ref() == Some(id))
        });
        match existing {
            Some(position) => merged[position].absorb(cluster),
            None => {
                cluster.known_id = id;
                merged.push(cluster);
            }
        }
    }
    *clusters = merged;
    clusters
        .iter()
        .filter(|cluster| cluster.known_id.is_some())
        .count()
}

/// A label with no embeddable speech is a blip: attach it to the embedded
/// cluster whose words sit nearest to its own, by majority over its words.
fn fold_unembedded(clusters: &mut Vec<Cluster>) {
    let has_anchor = clusters.iter().any(|cluster| cluster.centroid.is_some());
    if !has_anchor {
        return;
    }
    let mut anchors: Vec<Cluster> = Vec::new();
    let mut blips: Vec<Cluster> = Vec::new();
    for cluster in clusters.drain(..) {
        if cluster.centroid.is_some() {
            anchors.push(cluster);
        } else {
            blips.push(cluster);
        }
    }
    for blip in blips {
        let mut votes = vec![0usize; anchors.len()];
        for start in &blip.word_starts {
            let nearest = anchors
                .iter()
                .enumerate()
                .filter_map(|(index, anchor)| {
                    anchor
                        .word_starts
                        .iter()
                        .map(|other| (other - start).abs())
                        .min_by(f64::total_cmp)
                        .map(|distance| (index, distance))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((index, _)) = nearest {
                votes[index] += 1;
            }
        }
        let target = votes
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| **count)
            .map(|(index, _)| index)
            .unwrap_or(0);
        anchors[target].absorb(blip);
    }
    *clusters = anchors;
}

/// Only clusters with embedding evidence are merged; with none at all the
/// provider's labels stand, however many there are. Two clusters that matched
/// different known participants are never merged, even if that leaves the
/// channel over the cap: the count was wrong, the voiceprints were not.
fn merge_until_within_cap(clusters: &mut Vec<Cluster>, cap: usize) {
    while clusters.len() > cap.max(1) {
        let mut best: Option<(usize, usize, f32)> = None;
        for left in 0..clusters.len() {
            for right in (left + 1)..clusters.len() {
                if let (Some(a), Some(b)) = (&clusters[left].known_id, &clusters[right].known_id)
                    && a != b
                {
                    continue;
                }
                let (Some(a), Some(b)) = (&clusters[left].centroid, &clusters[right].centroid)
                else {
                    continue;
                };
                let Some(score) = cosine_similarity(a, b) else {
                    continue;
                };
                if best.is_none_or(|(_, _, best_score)| score > best_score) {
                    best = Some((left, right, score));
                }
            }
        }
        let Some((left, right, _)) = best else {
            break;
        };
        let other = clusters.remove(right);
        clusters[left].absorb(other);
    }
}

fn apply_relabels(
    response: &mut batch::Response,
    relabels: &[ChannelRelabel],
) -> Vec<ChannelConsolidation> {
    let mut summary = Vec::new();
    for relabel in relabels {
        let Some(channel) = response.results.channels.get_mut(relabel.index) else {
            continue;
        };
        let after = relabel
            .mapping
            .iter()
            .map(|(_, new)| *new)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        for word in channel
            .alternatives
            .iter_mut()
            .flat_map(|alternative| alternative.words.iter_mut())
        {
            if let Some(speaker) = word.speaker
                && let Some((_, new)) = relabel.mapping.iter().find(|(old, _)| *old == speaker)
            {
                word.speaker = Some(*new);
            }
        }
        summary.push(ChannelConsolidation {
            index: relabel.index,
            before: relabel.mapping.len(),
            after,
            identified: relabel.identified,
        });
    }

    if let Some(metadata) = response.metadata.as_object_mut() {
        metadata.insert(
            "speaker_consolidation".to_string(),
            serde_json::json!({
                "provider": CONSOLIDATION_PROVIDER,
                "channels": summary
                    .iter()
                    .map(|channel| serde_json::json!({
                        "index": channel.index,
                        "before": channel.before,
                        "after": channel.after,
                        "identified": channel.identified,
                    }))
                    .collect::<Vec<_>>(),
            }),
        );
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::super::test_fixtures::{
        NeverCancelled, Turn, english_1_coalesced_turns, params, stereo_fixture,
    };
    use super::*;
    use crate::batch::{BatchProvider, BatchRunMode};

    fn profile(label: usize, centroid: Option<Vec<f32>>, word_starts: Vec<f64>) -> SpeakerProfile {
        SpeakerProfile {
            label,
            weight: if centroid.is_some() { 1.0 } else { 0.0 },
            centroid,
            word_starts,
        }
    }

    #[test]
    fn under_cap_without_known_speakers_keeps_labels_and_renumbers() {
        let plan = consolidate_profiles(
            vec![
                profile(3, Some(vec![1.0, 0.0]), vec![0.0]),
                profile(1, Some(vec![0.0, 1.0]), vec![1.0]),
            ],
            Some(2),
            &[],
        );
        assert_eq!(plan.mapping, vec![(3, 0), (1, 1)]);
        assert_eq!(plan.identified, 0);
    }

    #[test]
    fn over_cap_merges_the_most_similar_pair_first() {
        let plan = consolidate_profiles(
            vec![
                profile(0, Some(vec![1.0, 0.0, 0.0]), vec![0.0]),
                profile(1, Some(vec![0.0, 1.0, 0.0]), vec![1.0]),
                profile(2, Some(vec![0.95, 0.05, 0.0]), vec![2.0]),
            ],
            Some(2),
            &[],
        );
        assert_eq!(plan.mapping, vec![(0, 0), (1, 1), (2, 0)]);
    }

    #[test]
    fn a_count_of_one_collapses_every_label() {
        let plan = consolidate_profiles(
            vec![
                profile(0, Some(vec![1.0, 0.0]), vec![0.0]),
                profile(1, Some(vec![0.0, 1.0]), vec![1.0]),
                profile(2, Some(vec![-1.0, 0.0]), vec![2.0]),
            ],
            Some(1),
            &[],
        );
        assert!(plan.mapping.iter().all(|(_, new)| *new == 0));
    }

    #[test]
    fn labels_matching_one_known_voice_merge_even_under_cap() {
        let ada = KnownSpeaker {
            id: "ada".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
        };
        let plan = consolidate_profiles(
            vec![
                profile(0, Some(vec![0.98, 0.1, 0.0]), vec![0.0]),
                profile(1, Some(vec![0.0, 1.0, 0.0]), vec![1.0]),
                profile(2, Some(vec![0.97, 0.0, 0.1]), vec![2.0]),
            ],
            Some(5),
            &[ada],
        );
        assert_eq!(plan.mapping, vec![(0, 0), (1, 1), (2, 0)]);
        assert_eq!(plan.identified, 1);
    }

    #[test]
    fn cap_merge_never_joins_two_known_participants() {
        let known = vec![
            KnownSpeaker {
                id: "ada".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
            KnownSpeaker {
                id: "bob".to_string(),
                embedding: vec![0.0, 1.0, 0.0],
            },
        ];
        // ada and bob are closer to each other than either is to the unknown
        // label, yet the cap of one may only fold the unknown label in.
        let plan = consolidate_profiles(
            vec![
                profile(0, Some(vec![0.9, 0.4, 0.0]), vec![0.0]),
                profile(1, Some(vec![0.4, 0.9, 0.0]), vec![1.0]),
                profile(2, Some(vec![0.0, 0.0, 1.0]), vec![2.0]),
            ],
            Some(1),
            &known,
        );
        assert_eq!(plan.mapping.len(), 3);
        assert_ne!(
            plan.mapping[0].1, plan.mapping[1].1,
            "known participants were merged: {:?}",
            plan.mapping
        );
        assert_eq!(plan.identified, 2);
    }

    #[test]
    fn ambiguous_known_matches_do_not_merge() {
        let known = vec![
            KnownSpeaker {
                id: "ada".to_string(),
                embedding: vec![1.0, 0.0],
            },
            KnownSpeaker {
                id: "bob".to_string(),
                embedding: vec![0.95, 0.31],
            },
        ];
        let plan = consolidate_profiles(
            vec![
                profile(0, Some(vec![1.0, 0.0]), vec![0.0]),
                profile(1, Some(vec![0.99, 0.14]), vec![1.0]),
            ],
            None,
            &known,
        );
        assert_eq!(plan.mapping, vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn unembeddable_labels_follow_their_nearest_neighbour_in_time() {
        let plan = consolidate_profiles(
            vec![
                profile(0, Some(vec![1.0, 0.0]), vec![0.0, 1.0, 2.0]),
                profile(1, Some(vec![0.0, 1.0]), vec![10.0, 11.0, 12.0]),
                profile(2, None, vec![10.4, 11.6]),
            ],
            Some(2),
            &[],
        );
        assert_eq!(plan.mapping, vec![(0, 0), (1, 1), (2, 1)]);
    }

    #[test]
    fn within_the_count_a_brief_talker_keeps_their_label_despite_known_voices() {
        let ada = KnownSpeaker {
            id: "ada".to_string(),
            embedding: vec![1.0, 0.0],
        };
        for cap in [Some(3), None] {
            let plan = consolidate_profiles(
                vec![
                    profile(0, Some(vec![1.0, 0.0]), vec![0.0, 1.0, 2.0]),
                    profile(1, Some(vec![0.0, 1.0]), vec![10.0, 11.0, 12.0]),
                    profile(2, None, vec![10.4, 11.6]),
                ],
                cap,
                &[ada.clone()],
            );
            assert_eq!(plan.mapping, vec![(0, 0), (1, 1), (2, 2)], "cap {cap:?}");
            assert_eq!(plan.identified, 1);
        }
    }

    #[test]
    fn nothing_to_embed_leaves_labels_alone() {
        let plan = consolidate_profiles(
            vec![profile(0, None, vec![0.0]), profile(1, None, vec![1.0])],
            Some(1),
            &[],
        );
        assert_eq!(plan.mapping, vec![(0, 0), (1, 1)]);
    }

    fn word(start: f64, end: f64, channel: i32, speaker: Option<usize>) -> batch::Word {
        batch::Word {
            word: "hi".to_string(),
            start,
            end,
            confidence: 1.0,
            channel,
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

    fn output(channels: Vec<batch::Channel>) -> BatchRunOutput {
        BatchRunOutput {
            session_id: "s".to_string(),
            mode: BatchRunMode::Direct,
            response: batch::Response {
                metadata: serde_json::json!({}),
                results: batch::Results { channels },
            },
        }
    }

    /// Words at ~400 ms for every turn `label_for` accepts, on `channel`.
    fn words_for_turns(
        turns: &[Turn],
        channel: i32,
        mut label_for: impl FnMut(usize, &Turn) -> Option<usize>,
    ) -> Vec<batch::Word> {
        turns
            .iter()
            .enumerate()
            .filter_map(|(index, turn)| label_for(index, turn).map(|label| (turn, label)))
            .flat_map(|(turn, label)| {
                let start = turn.start as f64 / 1000.0;
                let end = turn.end as f64 / 1000.0;
                let count = (((end - start) / 0.4).floor() as usize).max(1);
                let step = (end - start) / count as f64;
                (0..count).map(move |index| {
                    word(
                        start + step * index as f64,
                        start + step * (index + 1) as f64,
                        channel,
                        Some(label),
                    )
                })
            })
            .collect()
    }

    /// The remote channel of a stereo capture: speaker1's turns spread over
    /// `pieces` alternating provider labels, as if one voice were over-split.
    fn over_split_remote_channel(pieces: usize) -> batch::Channel {
        let turns = english_1_coalesced_turns();
        let mut remote_index = 0;
        channel(words_for_turns(&turns, 1, |_, turn| {
            (turn.speaker != "speaker0").then(|| {
                remote_index += 1;
                (remote_index - 1) % pieces
            })
        }))
    }

    #[tokio::test]
    async fn over_split_remote_speaker_collapses_to_the_participant_count() {
        let file = stereo_fixture();
        let mut params = params(
            BatchProvider::Deepgram,
            "https://api.deepgram.com/v1",
            file.path().to_str().unwrap(),
        );
        params.num_speakers = Some(2);
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(2),
            ..Default::default()
        };
        let remote = over_split_remote_channel(3);
        assert_eq!(distinct_labels(&remote).len(), 3);
        let mut output = output(vec![channel(vec![word(0.0, 1.0, 0, Some(0))]), remote]);

        consolidate_hosted_speakers(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        let labels = distinct_labels(&output.response.results.channels[1]);
        assert_eq!(
            labels,
            vec![0],
            "expected one remote speaker, got {labels:?}"
        );
        assert_eq!(
            output.response.metadata["speaker_consolidation"]["provider"],
            CONSOLIDATION_PROVIDER
        );
    }

    #[tokio::test]
    async fn over_cap_merge_joins_the_same_voice_not_the_other_speaker() {
        // A downmixed transcript of the stereo file: speaker0 split across
        // labels 0 and 1, speaker1 on label 2. Two participants means one
        // merge, and it has to be the two speaker0 labels.
        let file = stereo_fixture();
        let mut params = params(
            BatchProvider::Deepgram,
            "https://api.deepgram.com/v1",
            file.path().to_str().unwrap(),
        );
        params.num_speakers = Some(2);
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(2),
            ..Default::default()
        };
        let turns = english_1_coalesced_turns();
        let mut speaker0_index = 0;
        let words = words_for_turns(&turns, 0, |_, turn| {
            if turn.speaker == "speaker0" {
                speaker0_index += 1;
                Some((speaker0_index - 1) % 2)
            } else {
                Some(2)
            }
        });
        let mut output = output(vec![channel(words)]);

        consolidate_hosted_speakers(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        let words = &output.response.results.channels[0].alternatives[0].words;
        let label_at = |time_ms: u64| {
            let time = time_ms as f64 / 1000.0;
            words
                .iter()
                .find(|word| word.start <= time && time < word.end)
                .and_then(|word| word.speaker)
                .unwrap_or_else(|| panic!("no word at {time_ms} ms"))
        };
        let speaker0 = turns
            .iter()
            .filter(|turn| turn.speaker == "speaker0" && turn.end - turn.start >= 2_000)
            .map(|turn| label_at((turn.start + turn.end) / 2))
            .collect::<std::collections::BTreeSet<_>>();
        let speaker1 = turns
            .iter()
            .filter(|turn| turn.speaker != "speaker0" && turn.end - turn.start >= 2_000)
            .map(|turn| label_at((turn.start + turn.end) / 2))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(speaker0.len(), 1, "speaker0 labels: {speaker0:?}");
        assert_eq!(speaker1.len(), 1, "speaker1 labels: {speaker1:?}");
        assert_ne!(speaker0, speaker1);
    }

    #[tokio::test]
    async fn within_cap_without_known_speakers_does_not_touch_the_recording() {
        let params = params(
            BatchProvider::Deepgram,
            "https://api.deepgram.com/v1",
            "/nonexistent/recording.wav",
        );
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(4),
            ..Default::default()
        };
        let mut output = output(vec![
            channel(vec![word(0.0, 1.0, 0, Some(0))]),
            over_split_remote_channel(3),
        ]);
        let before = output.response.clone();

        consolidate_hosted_speakers(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        assert_eq!(output.response, before);
    }

    #[tokio::test]
    async fn local_output_is_left_to_the_diarizer() {
        let params = params(
            BatchProvider::Soniqo,
            "http://localhost:1234",
            "/nonexistent/recording.wav",
        );
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(2),
            ..Default::default()
        };
        let mut output = output(vec![
            channel(vec![word(0.0, 1.0, 0, Some(0))]),
            over_split_remote_channel(3),
        ]);
        let before = output.response.clone();

        consolidate_hosted_speakers(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        assert_eq!(output.response, before);
    }

    #[tokio::test]
    async fn unreadable_recording_keeps_the_transcript() {
        let params = params(
            BatchProvider::Deepgram,
            "https://api.deepgram.com/v1",
            "/nonexistent/recording.wav",
        );
        let listen_params = owhisper_interface::ListenParams {
            channels: 2,
            num_speakers: Some(2),
            ..Default::default()
        };
        let mut output = output(vec![
            channel(vec![word(0.0, 1.0, 0, Some(0))]),
            over_split_remote_channel(3),
        ]);
        let before = output.response.clone();

        consolidate_hosted_speakers(
            Arc::new(NeverCancelled),
            &params,
            &listen_params,
            &mut output,
        )
        .await;

        assert_eq!(output.response, before);
    }
}
