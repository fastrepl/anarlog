import { useMemo, useRef } from "react";

import type { Segment } from "~/stt/live-segment";

export function useStableSegments(segments: Segment[]): Segment[] {
  const cacheRef = useRef<Map<string, Segment>>(new Map());

  return useMemo(() => {
    const nextCache = new Map<string, Segment>();
    const stable = segments.map((segment) => {
      const cached = cacheRef.current.get(segment.id);
      if (cached && segmentsEqual(cached, segment)) {
        nextCache.set(segment.id, cached);
        return cached;
      }

      nextCache.set(segment.id, segment);
      return segment;
    });

    cacheRef.current = nextCache;
    return stable;
  }, [segments]);
}

export function createSegmentKey(
  segment: Segment,
  transcriptId: string,
  fallbackIndex: number,
) {
  return segment.id || `${transcriptId}-segment-${fallbackIndex}`;
}

function segmentsEqual(a: Segment, b: Segment) {
  if (
    a.id !== b.id ||
    a.start_ms !== b.start_ms ||
    a.end_ms !== b.end_ms ||
    a.text !== b.text ||
    a.key.channel !== b.key.channel ||
    a.key.speaker_index !== b.key.speaker_index ||
    a.key.speaker_human_id !== b.key.speaker_human_id ||
    a.words.length !== b.words.length
  ) {
    return false;
  }

  for (let index = 0; index < a.words.length; index += 1) {
    const aw = a.words[index]!;
    const bw = b.words[index]!;
    if (
      aw.id !== bw.id ||
      aw.text !== bw.text ||
      aw.start_ms !== bw.start_ms ||
      aw.end_ms !== bw.end_ms ||
      aw.channel !== bw.channel ||
      aw.is_final !== bw.is_final
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
