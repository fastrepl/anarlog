export type TimelineTranscriptRow = {
  session_id?: string | null;
  started_at?: number | null;
  ended_at?: number | null;
  words?: string | null;
};

export type TimelineTranscriptsTable =
  | Record<string, TimelineTranscriptRow>
  | null
  | undefined;

export type SessionRecordingRange = {
  start: Date;
  end: Date;
};

export function buildSessionRecordingRanges(
  transcriptsTable: TimelineTranscriptsTable,
): Map<string, SessionRecordingRange> {
  const rangesBySession = new Map<string, { startMs: number; endMs: number }>();

  if (!transcriptsTable) {
    return new Map();
  }

  Object.values(transcriptsTable).forEach((row) => {
    if (!row.session_id || typeof row.started_at !== "number") {
      return;
    }

    const startMs = row.started_at;
    const endMs = getTranscriptEndMs(row, startMs);

    if (!Number.isFinite(startMs) || !endMs || endMs <= startMs) {
      return;
    }

    const existing = rangesBySession.get(row.session_id);
    if (!existing) {
      rangesBySession.set(row.session_id, { startMs, endMs });
      return;
    }

    existing.startMs = Math.min(existing.startMs, startMs);
    existing.endMs = Math.max(existing.endMs, endMs);
  });

  return new Map(
    [...rangesBySession.entries()].map(([sessionId, range]) => [
      sessionId,
      {
        start: new Date(range.startMs),
        end: new Date(range.endMs),
      },
    ]),
  );
}

function getTranscriptEndMs(
  row: TimelineTranscriptRow,
  startedAtMs: number,
): number | null {
  if (typeof row.ended_at === "number" && row.ended_at > startedAtMs) {
    return row.ended_at;
  }

  const wordsEndMs = getWordsEndMs(row.words);
  if (!wordsEndMs) {
    return null;
  }

  return startedAtMs + wordsEndMs;
}

function getWordsEndMs(wordsJson?: string | null): number | null {
  if (!wordsJson) {
    return null;
  }

  try {
    const words = JSON.parse(wordsJson) as Array<{ end_ms?: unknown }>;
    const maxEndMs = words.reduce((max, word) => {
      if (typeof word.end_ms !== "number" || !Number.isFinite(word.end_ms)) {
        return max;
      }

      return Math.max(max, word.end_ms);
    }, 0);

    return maxEndMs > 0 ? maxEndMs : null;
  } catch {
    return null;
  }
}
