import { useMemo, useRef } from "react";

import { buildSegments, type Segment } from "@hypr/transcript";

type SegmentsBuilder = typeof buildSegments;

export const useStableSegments: SegmentsBuilder = (
  finalWords,
  partialWords,
  speakerHints,
  options,
) => {
  const cacheRef = useRef<Map<string, Segment>>(new Map());

  return useMemo(() => {
    const fresh = buildSegments(
      finalWords,
      partialWords,
      speakerHints,
      options,
    );
    const nextCache = new Map<string, Segment>();

    const segments = fresh.map((segment) => {
      const key = createStableSegmentKey(segment);
      const cached = cacheRef.current.get(key);

      if (cached && segmentsDeepEqual(cached, segment)) {
        nextCache.set(key, cached);
        return cached;
      }

      nextCache.set(key, segment);
      return segment;
    });

    cacheRef.current = nextCache;
    return segments;
  }, [finalWords, partialWords, speakerHints, options]);
};

function createStableSegmentKey(segment: Segment) {
  const firstWord = segment.words[0];
  const lastWord = segment.words[segment.words.length - 1];

  const firstAnchor = firstWord
    ? (firstWord.id ?? `start:${firstWord.start_ms}`)
    : "none";

  const lastAnchor = lastWord
    ? (lastWord.id ?? `end:${lastWord.end_ms}`)
    : "none";

  return [
    segment.key.channel,
    segment.key.speaker_index ?? "none",
    segment.key.speaker_human_id ?? "none",
    firstAnchor,
    lastAnchor,
  ].join(":");
}

export function createSegmentKey(
  segment: Segment,
  transcriptId: string,
  fallbackIndex: number,
) {
  const stableKey = createStableSegmentKey(segment);
  if (segment.words.length === 0) {
    return [transcriptId, stableKey, `index:${fallbackIndex}`].join("-");
  }

  return [transcriptId, stableKey].join("-");
}

function segmentsDeepEqual(a: Segment, b: Segment) {
  if (
    a.key.channel !== b.key.channel ||
    a.key.speaker_index !== b.key.speaker_index ||
    a.key.speaker_human_id !== b.key.speaker_human_id ||
    a.words.length !== b.words.length
  ) {
    return false;
  }

  for (let index = 0; index < a.words.length; index += 1) {
    const aw = a.words[index];
    const bw = b.words[index];

    if (
      aw.id !== bw.id ||
      aw.text !== bw.text ||
      aw.start_ms !== bw.start_ms ||
      aw.end_ms !== bw.end_ms ||
      aw.channel !== bw.channel ||
      aw.isFinal !== bw.isFinal
    ) {
      return false;
    }
  }

  return true;
}

export function segmentsShallowEqual(a: Segment[], b: Segment[]) {
  if (a === b) {
    return true;
  }

  if (a.length !== b.length) {
    return false;
  }

  for (let index = 0; index < a.length; index += 1) {
    if (a[index] !== b[index]) {
      return false;
    }
  }

  return true;
}
