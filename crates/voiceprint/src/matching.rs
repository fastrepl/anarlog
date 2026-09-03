//! Conservative identity matching for named voiceprints.

pub const MIN_UNIQUE_SCORE: f32 = 0.62;
pub const MIN_UNIQUE_MARGIN: f32 = 0.08;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceprintSpeakerKey {
    pub channel: i64,
    pub speaker_index: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceprintAssignment {
    pub speaker: VoiceprintSpeakerKey,
    pub human_id: String,
    pub score: f32,
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left_value, right_value) in left.iter().zip(right) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }

    let denom = left_norm.sqrt() * right_norm.sqrt();
    if denom == 0.0 {
        return None;
    }

    Some((dot / denom).clamp(-1.0, 1.0))
}

pub fn pick_unique_voiceprint_assignments(
    scores: &[(VoiceprintSpeakerKey, &str, f32)],
    min_score: f32,
    min_margin: f32,
) -> Vec<VoiceprintAssignment> {
    let mut best_human_by_speaker: std::collections::HashMap<
        VoiceprintSpeakerKey,
        (String, f32, f32),
    > = std::collections::HashMap::new();
    let mut best_speaker_by_human: std::collections::HashMap<
        &str,
        (VoiceprintSpeakerKey, f32, f32),
    > = std::collections::HashMap::new();

    for (speaker, human_id, score) in scores {
        update_best_pair(
            best_human_by_speaker
                .entry(*speaker)
                .or_insert_with(|| (human_id.to_string(), f32::NEG_INFINITY, f32::NEG_INFINITY)),
            *human_id,
            *score,
        );
        update_best_speaker(
            best_speaker_by_human.entry(*human_id).or_insert((
                *speaker,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            )),
            *speaker,
            *score,
        );
    }

    let mut assignments = Vec::new();
    for (speaker, (human_id, best, second)) in best_human_by_speaker {
        if !is_unique_best(best, second, min_score, min_margin) {
            continue;
        }
        let Some((best_speaker, human_best, human_second)) =
            best_speaker_by_human.get(human_id.as_str())
        else {
            continue;
        };
        if *best_speaker != speaker
            || !is_unique_best(*human_best, *human_second, min_score, min_margin)
        {
            continue;
        }

        assignments.push(VoiceprintAssignment {
            speaker,
            human_id,
            score: best,
        });
    }

    assignments.sort_by(|left, right| {
        left.speaker
            .channel
            .cmp(&right.speaker.channel)
            .then(left.speaker.speaker_index.cmp(&right.speaker.speaker_index))
            .then(left.human_id.cmp(&right.human_id))
    });
    assignments
}

fn update_best_pair(entry: &mut (String, f32, f32), human_id: &str, score: f32) {
    if score > entry.1 {
        if entry.0 != human_id {
            entry.2 = entry.1;
        }
        entry.0 = human_id.to_string();
        entry.1 = score;
    } else if entry.0 != human_id && score > entry.2 {
        entry.2 = score;
    }
}

fn update_best_speaker(
    entry: &mut (VoiceprintSpeakerKey, f32, f32),
    speaker: VoiceprintSpeakerKey,
    score: f32,
) {
    if score > entry.1 {
        if entry.0 != speaker {
            entry.2 = entry.1;
        }
        entry.0 = speaker;
        entry.1 = score;
    } else if entry.0 != speaker && score > entry.2 {
        entry.2 = score;
    }
}

fn is_unique_best(best: f32, second: f32, min_score: f32, min_margin: f32) -> bool {
    best >= min_score && (second.is_infinite() || best - second >= min_margin)
}

/// When a channel holds exactly as many speakers as there are people who could
/// be on it, and every speaker but one and every person but one is already
/// paired, the last speaker is the last person. The count must match exactly:
/// a no-show or an uninvited guest breaks the inference, so nothing is guessed.
pub fn complete_by_elimination(
    speakers: &[VoiceprintSpeakerKey],
    people: &[String],
    assigned_speakers: &[VoiceprintSpeakerKey],
    assigned_people: &[&str],
) -> Option<(VoiceprintSpeakerKey, String)> {
    if speakers.is_empty() || speakers.len() != people.len() {
        return None;
    }
    let mut open_speakers = speakers
        .iter()
        .filter(|speaker| !assigned_speakers.contains(speaker));
    let mut open_people = people
        .iter()
        .filter(|person| !assigned_people.contains(&person.as_str()));
    let (speaker, person) = (open_speakers.next()?, open_people.next()?);
    if open_speakers.next().is_some() || open_people.next().is_some() {
        return None;
    }
    Some((*speaker, person.clone()))
}

pub fn remote_participant_human_ids<'a>(
    participants: impl IntoIterator<Item = (&'a str, &'a str)>,
    owner_user_id: &str,
    owner_email: Option<&str>,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for (human_id, email) in participants {
        if human_id.is_empty() || human_id == owner_user_id {
            continue;
        }
        if let Some(owner_email) = owner_email
            && !email.is_empty()
            && email.eq_ignore_ascii_case(owner_email)
        {
            continue;
        }
        if seen.insert(human_id.to_string()) {
            ids.push(human_id.to_string());
        }
    }
    ids
}

pub fn collect_match_scores(
    samples: &[(VoiceprintSpeakerKey, &str, &[f32])],
    exemplars: &[(String, String, Vec<f32>)],
) -> Vec<(VoiceprintSpeakerKey, String, f32)> {
    let mut best = std::collections::HashMap::<(VoiceprintSpeakerKey, String), f32>::new();
    for (speaker, domain, embedding) in samples {
        for (human_id, exemplar_domain, exemplar) in exemplars {
            if exemplar_domain != domain {
                continue;
            }
            let Some(score) = cosine_similarity(embedding, exemplar) else {
                continue;
            };
            let entry = best
                .entry((*speaker, human_id.clone()))
                .or_insert(f32::NEG_INFINITY);
            if score > *entry {
                *entry = score;
            }
        }
    }
    best.into_iter()
        .map(|((speaker, human_id), score)| (speaker, human_id, score))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speaker(channel: i64, index: i64) -> VoiceprintSpeakerKey {
        VoiceprintSpeakerKey {
            channel,
            speaker_index: Some(index),
        }
    }

    #[test]
    fn cosine_rejects_mismatched_or_empty_vectors() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 0.0]), None);
        assert_eq!(cosine_similarity(&[], &[]), None);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), None);
    }

    #[test]
    fn cosine_is_one_for_identical_vectors() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]), Some(1.0));
    }

    #[test]
    fn assigns_a_mutual_unique_match() {
        let marco = speaker(1, 0);
        let ada = speaker(1, 1);
        let assignments = pick_unique_voiceprint_assignments(
            &[
                (marco, "marco", 0.84),
                (marco, "ada", 0.41),
                (ada, "ada", 0.81),
                (ada, "marco", 0.38),
            ],
            MIN_UNIQUE_SCORE,
            MIN_UNIQUE_MARGIN,
        );

        assert_eq!(
            assignments
                .iter()
                .map(|assignment| (assignment.speaker, assignment.human_id.as_str()))
                .collect::<Vec<_>>(),
            vec![(marco, "marco"), (ada, "ada")]
        );
    }

    #[test]
    fn does_not_guess_when_two_speakers_want_the_same_person() {
        let first = speaker(1, 0);
        let second = speaker(1, 1);
        assert!(
            pick_unique_voiceprint_assignments(
                &[(first, "marco", 0.88), (second, "marco", 0.86)],
                MIN_UNIQUE_SCORE,
                MIN_UNIQUE_MARGIN,
            )
            .is_empty()
        );
    }

    #[test]
    fn does_not_guess_when_the_margin_is_thin() {
        let remote = speaker(1, 0);
        assert!(
            pick_unique_voiceprint_assignments(
                &[(remote, "marco", 0.71), (remote, "ada", 0.68)],
                MIN_UNIQUE_SCORE,
                MIN_UNIQUE_MARGIN,
            )
            .is_empty()
        );
    }

    #[test]
    fn does_not_guess_below_the_score_floor() {
        let remote = speaker(1, 0);
        assert!(
            pick_unique_voiceprint_assignments(
                &[(remote, "marco", 0.51)],
                MIN_UNIQUE_SCORE,
                MIN_UNIQUE_MARGIN,
            )
            .is_empty()
        );
    }

    #[test]
    fn elimination_names_the_last_speaker_when_counts_match() {
        let speakers = [speaker(1, 0), speaker(1, 1), speaker(1, 2)];
        let people = ["ada".to_string(), "bob".to_string(), "cy".to_string()];

        assert_eq!(
            complete_by_elimination(
                &speakers,
                &people,
                &[speaker(1, 0), speaker(1, 2)],
                &["cy", "ada"],
            ),
            Some((speaker(1, 1), "bob".to_string()))
        );
    }

    #[test]
    fn elimination_needs_exactly_one_open_speaker_and_person() {
        let speakers = [speaker(1, 0), speaker(1, 1), speaker(1, 2)];
        let people = ["ada".to_string(), "bob".to_string(), "cy".to_string()];

        assert_eq!(
            complete_by_elimination(&speakers, &people, &[speaker(1, 0)], &["ada"]),
            None
        );
        assert_eq!(
            complete_by_elimination(&speakers, &people, &speakers, &["ada", "bob", "cy"]),
            None
        );
    }

    #[test]
    fn elimination_refuses_when_speaker_and_participant_counts_differ() {
        let people = ["ada".to_string(), "bob".to_string()];

        // A no-show: two invited, one spoke.
        assert_eq!(
            complete_by_elimination(&[speaker(1, 0)], &people, &[], &[]),
            None
        );
        // An uninvited guest: two invited, three spoke, one already named.
        assert_eq!(
            complete_by_elimination(
                &[speaker(1, 0), speaker(1, 1), speaker(1, 2)],
                &people,
                &[speaker(1, 0)],
                &["ada"],
            ),
            None
        );
        assert_eq!(complete_by_elimination(&[], &[], &[], &[]), None);
    }

    #[test]
    fn remote_participants_drop_owner_and_self_email_copies() {
        assert_eq!(
            remote_participant_human_ids(
                [
                    ("john", "john@example.com"),
                    ("john-cal", "john@example.com"),
                    ("marco", "marco@example.com"),
                    ("ada", ""),
                    ("", "nobody@example.com"),
                ],
                "john",
                Some("john@example.com"),
            ),
            vec!["marco".to_string(), "ada".to_string()]
        );
    }

    #[test]
    fn match_scores_keep_same_domain_and_the_best_sample() {
        let remote = speaker(1, 0);
        let scores = collect_match_scores(
            &[
                (remote, "system_audio", &[1.0, 0.0]),
                (remote, "system_audio", &[0.8, 0.2]),
            ],
            &[
                (
                    "marco".to_string(),
                    "direct_mic".to_string(),
                    vec![1.0, 0.0],
                ),
                (
                    "ada".to_string(),
                    "system_audio".to_string(),
                    vec![1.0, 0.0],
                ),
            ],
        );

        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].0, remote);
        assert_eq!(scores[0].1, "ada");
        assert!((scores[0].2 - 1.0).abs() < 1e-6);
    }
}
