import type {
  ZoomActiveSpeakers,
  ZoomClientEvent,
  ZoomParticipant,
} from "@anlg/plugin-zoom-client";

import type { WordWithId } from "~/stt/types";

// Mirrors `anlg_transcript::word_assignments_from_active_speakers`: each word is
// attributed to the participant whose active-speaker intervals overlap it the most.

export type ZoomSpeakerTimeline = {
  captureStartedAtMs: number | null;
  participants: Map<string, ZoomParticipant>;
  samples: ZoomActiveSpeakers[];
};

export function createZoomSpeakerTimeline(): ZoomSpeakerTimeline {
  return { captureStartedAtMs: null, participants: new Map(), samples: [] };
}

export function applyZoomClientEvent(
  timeline: ZoomSpeakerTimeline,
  event: ZoomClientEvent,
  now: number = Date.now(),
): void {
  switch (event.type) {
    case "state_changed":
      if (
        event.data.state === "capturing" &&
        timeline.captureStartedAtMs == null
      ) {
        timeline.captureStartedAtMs = now;
      }
      break;
    case "participant_upserted":
      timeline.participants.set(
        event.data.participant.id,
        event.data.participant,
      );
      break;
    case "active_speakers":
      timeline.samples.push(event.data.speakers);
      break;
    case "participant_left":
    case "error":
      break;
  }
}

export function wordIdsByParticipant(
  words: readonly WordWithId[],
  transcriptStartedAtMs: number,
  timeline: ZoomSpeakerTimeline,
): Map<string, string[]> {
  const result = new Map<string, string[]>();
  if (timeline.captureStartedAtMs == null || timeline.samples.length === 0) {
    return result;
  }

  const offset = transcriptStartedAtMs - timeline.captureStartedAtMs;
  const samples = [...timeline.samples].sort((a, b) => a.at_ms - b.at_ms);

  for (const word of words) {
    if (word.start_ms == null) continue;
    const start = word.start_ms + offset;
    const end = Math.max(word.end_ms ?? 0, word.start_ms + 1) + offset;
    const coverage = new Map<string, number>();

    for (let i = 0; i < samples.length; i += 1) {
      const sample = samples[i];
      const sampleEnd = samples[i + 1]?.at_ms ?? Number.POSITIVE_INFINITY;
      const overlap = Math.min(sampleEnd, end) - Math.max(sample.at_ms, start);
      if (overlap <= 0) continue;
      for (const id of sample.participant_ids) {
        coverage.set(id, (coverage.get(id) ?? 0) + overlap);
      }
    }

    let best: [string, number] | null = null;
    for (const entry of coverage) {
      if (!best || entry[1] > best[1]) best = entry;
    }
    if (!best) continue;
    const ids = result.get(best[0]) ?? [];
    ids.push(word.id);
    result.set(best[0], ids);
  }

  return result;
}
