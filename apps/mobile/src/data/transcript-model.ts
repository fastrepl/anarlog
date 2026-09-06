export const SESSION_TRANSCRIPTS_SQL = `SELECT transcript.id, transcript.started_at_ms, transcript.words_json, transcript.speaker_hints_json,
      COALESCE((SELECT json_group_array(json(ordered_delta.delta_json)) FROM (
        SELECT delta.delta_json FROM transcript_live_deltas AS delta
        WHERE delta.transcript_id = transcript.id ORDER BY delta.sequence
      ) AS ordered_delta), '[]') AS pending_deltas_json
      FROM transcripts AS transcript WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
      ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id`;

export const SESSION_SPEAKERS_SQL = `SELECT id, name FROM humans WHERE workspace_id = (SELECT workspace_id FROM sessions WHERE id = ?) AND deleted_at IS NULL`;

export type TranscriptRow = {
  id: string;
  started_at_ms: number;
  words_json: string;
  speaker_hints_json: string;
  pending_deltas_json: string;
};

export type TranscriptSegment = {
  id: string;
  text: string;
  speaker: string;
  startMs: number;
};

type Word = {
  id: string;
  text: string;
  start_ms: number;
  end_ms: number;
  channel: number;
  speaker_index?: number | null;
};

function parseArray(json: string): unknown[] {
  const value: unknown = JSON.parse(json);
  if (!Array.isArray(value)) throw new Error("Invalid transcript data");
  return value;
}

export function transcriptSegments(
  row: TranscriptRow,
  names: ReadonlyMap<string, string> = new Map(),
): TranscriptSegment[] {
  const wordsById = new Map<string, Word>();
  const addWords = (values: unknown[]) => {
    for (const [index, value] of values.entries()) {
      if (!value || typeof value !== "object") continue;
      const word = value as Partial<Word>;
      if (typeof word.text !== "string") continue;
      const id = typeof word.id === "string" ? word.id : `${row.id}:${index}`;
      wordsById.set(id, {
        id,
        text: word.text,
        start_ms: typeof word.start_ms === "number" ? word.start_ms : 0,
        end_ms: typeof word.end_ms === "number" ? word.end_ms : 0,
        channel: typeof word.channel === "number" ? word.channel : 0,
        speaker_index: word.speaker_index,
      });
    }
  };
  addWords(parseArray(row.words_json));
  const updatedWordIds = new Set<string>();
  for (const value of parseArray(row.pending_deltas_json)) {
    if (!value || typeof value !== "object") continue;
    const delta = value as { replaced_ids?: string[]; new_words?: unknown[] };
    if (Array.isArray(delta.replaced_ids)) {
      for (const id of delta.replaced_ids) wordsById.delete(id);
    }
    if (Array.isArray(delta.new_words)) {
      addWords(delta.new_words);
      for (const word of delta.new_words) {
        if (
          word &&
          typeof word === "object" &&
          "id" in word &&
          typeof word.id === "string"
        )
          updatedWordIds.add(word.id);
      }
    }
  }
  const words = [...wordsById.values()].sort(
    (a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms,
  );
  const hints = parseArray(row.speaker_hints_json).flatMap((hint) => {
    if (!hint || typeof hint !== "object") return [];
    const item = hint as { type?: string; word_id?: string; value?: unknown };
    const value =
      typeof item.value === "string" ? JSON.parse(item.value) : item.value;
    return value && typeof value === "object" ? [{ ...item, value }] : [];
  });
  for (const hint of hints) {
    if (hint.type !== "provider_speaker_index" || !hint.word_id) continue;
    const word = wordsById.get(hint.word_id);
    const value = hint.value as { channel?: number; speaker_index?: number };
    // A live revision can reuse word IDs while correcting the initial speaker hint.
    if (word && updatedWordIds.has(word.id) && word.speaker_index != null)
      continue;
    if (word && typeof value.speaker_index === "number") {
      word.speaker_index = value.speaker_index;
      if (typeof value.channel === "number") word.channel = value.channel;
    }
  }
  const identities = new Map<string, string>();
  // Desktop applies explicit user assignments after automatic speaker matching.
  for (const type of [
    "automatic_speaker_assignment",
    "user_speaker_assignment",
  ]) {
    for (const hint of hints) {
      if (hint.type !== type) continue;
      const value = hint.value as {
        human_id?: string;
        scope?: string;
        channel?: number;
        speaker_index?: number;
        word_ids?: string[];
      };
      if (!value.human_id) continue;
      const anchor = hint.word_id ? wordsById.get(hint.word_id) : undefined;
      for (const word of words) {
        const matches =
          value.scope === "segment"
            ? value.word_ids?.includes(word.id)
            : value.scope === "speaker"
              ? word.channel === value.channel &&
                (value.speaker_index == null ||
                  word.speaker_index === value.speaker_index)
              : anchor &&
                word.channel === anchor.channel &&
                (anchor.speaker_index == null ||
                  word.speaker_index === anchor.speaker_index);
        if (matches) identities.set(word.id, value.human_id);
      }
    }
  }
  const result: (Omit<TranscriptSegment, "text"> & { parts: string[] })[] = [];
  let previousIdentity: string | undefined;
  for (const word of words) {
    if (!word.text.trim()) continue;
    const humanId = identities.get(word.id);
    const identity =
      humanId ?? `${word.channel}:${word.speaker_index ?? "unknown"}`;
    const speaker =
      (humanId ? names.get(humanId) : undefined) ||
      (typeof word.speaker_index === "number"
        ? `Speaker ${word.speaker_index + 1}`
        : "Speaker");
    const last = result.at(-1);
    if (last && previousIdentity === identity) {
      last.parts.push(word.text);
    } else {
      result.push({
        id: `${row.id}:${word.id}`,
        parts: [word.text],
        speaker,
        startMs: row.started_at_ms + word.start_ms,
      });
    }
    previousIdentity = identity;
  }
  return result.map(({ parts, ...segment }) => ({
    ...segment,
    text: parts.join(" ").replace(/\s+/g, " ").trim(),
  }));
}
