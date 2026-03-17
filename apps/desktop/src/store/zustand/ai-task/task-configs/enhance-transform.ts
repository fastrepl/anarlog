import type {
  EnhanceTemplate,
  Participant,
  Segment,
  Session,
  TemplateSection,
  Transcript,
} from "@hypr/plugin-template";

import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";

import { getSessionEventById } from "~/session/utils";
import type { Store as MainStore } from "~/store/tinybase/store/main";
import type { Store as SettingsStore } from "~/store/tinybase/store/settings";
import {
  buildSegments,
  type RuntimeSpeakerHint,
  SegmentKey,
  type WordLike,
} from "~/stt/segment";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";
import { convertStorageHintsToRuntime } from "~/stt/speaker-hints";

type TranscriptMeta = {
  id: string;
  startedAt: number;
  endedAt: number | null;
  memoMd: string;
};

type WordRow = Record<string, unknown> & {
  text: string;
  start_ms: number;
  end_ms: number;
  channel: WordLike["channel"];
  transcript_id: string;
  is_final?: boolean;
  id?: string;
};

type WordWithTranscript = WordRow & { transcriptStartedAt: number };

type SegmentForPayload = {
  key: SegmentKey;
  words: WordWithTranscript[];
};

type SegmentPayload = {
  speaker_label: string;
  start_ms: number;
  end_ms: number;
  text: string;
  words: Array<{ text: string; start_ms: number; end_ms: number }>;
};

export const enhanceTransform: Pick<TaskConfig<"enhance">, "transformArgs"> = {
  transformArgs,
};

async function transformArgs(
  args: TaskArgsMap["enhance"],
  store: MainStore,
  settingsStore: SettingsStore,
): Promise<TaskArgsMapTransformed["enhance"]> {
  const { sessionId, templateId } = args;

  const sessionContext = getSessionContext(sessionId, store);
  const template = templateId ? getTemplateData(templateId, store) : null;
  const language = getLanguage(settingsStore);

  return {
    language,
    session: sessionContext.session,
    participants: sessionContext.participants,
    template,
    preMeetingMemo: sessionContext.preMeetingMemo,
    postMeetingMemo: sessionContext.postMeetingMemo,
    transcripts: formatTranscripts(
      sessionContext.segments,
      sessionContext.transcriptsMeta,
    ),
  };
}

function formatTranscripts(
  segments: SegmentPayload[],
  transcriptsMeta: TranscriptMeta[],
): Transcript[] {
  if (segments.length > 0 && transcriptsMeta.length > 0) {
    const startedAt = transcriptsMeta.reduce(
      (min, t) => Math.min(min, t.startedAt),
      Number.POSITIVE_INFINITY,
    );
    const endedAt = transcriptsMeta.reduce(
      (max, t) => Math.max(max, t.endedAt ?? t.startedAt),
      Number.NEGATIVE_INFINITY,
    );

    return [
      {
        segments: segments.map(
          (s): Segment => ({
            speaker: s.speaker_label,
            text: s.text,
          }),
        ),
        startedAt: Number.isFinite(startedAt) ? startedAt : null,
        endedAt: Number.isFinite(endedAt) ? endedAt : null,
      },
    ];
  }

  return [];
}

function getLanguage(settingsStore: SettingsStore): string | null {
  const value = settingsStore.getValue("ai_language");
  return typeof value === "string" && value.length > 0 ? value : null;
}

function getSessionContext(sessionId: string, store: MainStore) {
  const transcriptsMeta = collectTranscripts(sessionId, store);
  const rawMd = getStringCell(store, "sessions", sessionId, "raw_md");

  const earliest =
    transcriptsMeta.length > 0
      ? transcriptsMeta.reduce((a, b) => (a.startedAt <= b.startedAt ? a : b))
      : null;
  const preMeetingMemo = earliest?.memoMd ?? "";

  return {
    preMeetingMemo,
    postMeetingMemo: rawMd,
    session: getSessionData(sessionId, store),
    participants: getParticipants(sessionId, store),
    segments: getTranscriptSegmentsFromMeta(transcriptsMeta, store),
    transcriptsMeta,
  };
}

function getSessionData(sessionId: string, store: MainStore): Session {
  const rawTitle = getStringCell(store, "sessions", sessionId, "title");
  const parsed = getSessionEventById(store, sessionId);

  if (parsed) {
    const eventTitle = parsed.title;
    return {
      title: eventTitle || rawTitle || null,
      startedAt: parsed.started_at ?? null,
      endedAt: parsed.ended_at ?? null,
      event: {
        name: eventTitle || rawTitle || "",
      },
    };
  }

  return {
    title: rawTitle || null,
    startedAt: null,
    endedAt: null,
    event: null,
  };
}

function getParticipants(sessionId: string, store: MainStore): Participant[] {
  const participants: Participant[] = [];

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mappingSessionId = getOptionalStringCell(
      store,
      "mapping_session_participant",
      mappingId,
      "session_id",
    );
    if (mappingSessionId !== sessionId) {
      return;
    }

    const humanId = getOptionalStringCell(
      store,
      "mapping_session_participant",
      mappingId,
      "human_id",
    );
    if (!humanId) {
      return;
    }

    const name = getStringCell(store, "humans", humanId, "name");
    if (!name) {
      return;
    }

    participants.push({
      name,
      jobTitle:
        getOptionalStringCell(store, "humans", humanId, "job_title") ?? null,
    });
  });

  return participants;
}

function getTemplateData(
  templateId: string,
  store: MainStore,
): EnhanceTemplate {
  return {
    title: getStringCell(store, "templates", templateId, "title"),
    description:
      getOptionalStringCell(store, "templates", templateId, "description") ??
      null,
    sections: parseTemplateSections(
      store.getCell("templates", templateId, "sections"),
    ),
  };
}

function parseTemplateSections(raw: unknown): TemplateSection[] {
  let value: unknown = raw;

  if (typeof raw === "string") {
    try {
      value = JSON.parse(raw);
    } catch {
      return [];
    }
  }

  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((section): TemplateSection | null => {
      if (typeof section === "string") {
        return { title: section, description: null };
      }

      if (section && typeof section === "object") {
        const record = section as Record<string, unknown>;
        const title =
          typeof record.title === "string" ? record.title.trim() : "";
        if (!title) {
          return null;
        }

        const description =
          typeof record.description === "string" ? record.description : null;
        return { title, description };
      }

      return null;
    })
    .filter((section): section is TemplateSection => section !== null);
}

function getTranscriptSegmentsFromMeta(
  transcripts: TranscriptMeta[],
  store: MainStore,
) {
  if (transcripts.length === 0) {
    return [];
  }

  const wordIdToIndex = new Map<string, number>();
  const words = collectWordsForTranscripts(store, transcripts, wordIdToIndex);
  if (words.length === 0) {
    return [];
  }

  const speakerHints = collectSpeakerHints(store, transcripts, wordIdToIndex);
  const segments = buildSegments(words, [], speakerHints);

  const ctx = defaultRenderLabelContext(store);
  const speakerLabelManager = SpeakerLabelManager.fromSegments(segments, ctx);

  const sessionStartCandidate = transcripts.reduce(
    (min, transcript) => Math.min(min, transcript.startedAt),
    Number.POSITIVE_INFINITY,
  );
  const sessionStartMs = Number.isFinite(sessionStartCandidate)
    ? sessionStartCandidate
    : 0;

  const segmentsForPayload = segments as unknown as SegmentForPayload[];

  const normalizedSegments = segmentsForPayload.reduce<SegmentPayload[]>(
    (acc, segment) => {
      if (segment.words.length === 0) {
        return acc;
      }

      acc.push(
        toSegmentPayload(segment, sessionStartMs, store, speakerLabelManager),
      );
      return acc;
    },
    [],
  );

  return normalizedSegments.sort((a, b) => a.start_ms - b.start_ms);
}

function collectTranscripts(
  sessionId: string,
  store: MainStore,
): TranscriptMeta[] {
  const transcripts: TranscriptMeta[] = [];

  store.forEachRow("transcripts", (transcriptId, _forEachCell) => {
    const transcriptSessionId = getOptionalStringCell(
      store,
      "transcripts",
      transcriptId,
      "session_id",
    );
    if (transcriptSessionId !== sessionId) {
      return;
    }

    const startedAt =
      getNumberCell(store, "transcripts", transcriptId, "started_at") ?? 0;
    const endedAt =
      getNumberCell(store, "transcripts", transcriptId, "ended_at") ?? null;
    const memoMd = getStringCell(store, "transcripts", transcriptId, "memo_md");
    transcripts.push({ id: transcriptId, startedAt, endedAt, memoMd });
  });

  return transcripts;
}

function collectWordsForTranscripts(
  store: MainStore,
  transcripts: readonly TranscriptMeta[],
  wordIdToIndex: Map<string, number>,
): WordWithTranscript[] {
  const words: Array<{ id: string; word: WordWithTranscript }> = [];

  for (const transcript of transcripts) {
    const wordsJson = store.getCell("transcripts", transcript.id, "words");
    if (typeof wordsJson !== "string") continue;

    let parsedWords: Array<WordRow & { id: string }>;
    try {
      parsedWords = JSON.parse(wordsJson);
    } catch {
      continue;
    }

    for (const word of parsedWords) {
      words.push({
        id: word.id,
        word: {
          ...word,
          transcript_id: transcript.id,
          transcriptStartedAt: transcript.startedAt,
        },
      });
    }
  }

  words.sort((a, b) => {
    const startA = a.word.transcriptStartedAt + a.word.start_ms;
    const startB = b.word.transcriptStartedAt + b.word.start_ms;
    return startA - startB;
  });

  return words.map(({ id, word }, index) => {
    wordIdToIndex.set(id, index);
    return word;
  });
}

function collectSpeakerHints(
  store: MainStore,
  transcripts: readonly TranscriptMeta[],
  wordIdToIndex: Map<string, number>,
): RuntimeSpeakerHint[] {
  const storageHints: any[] = [];

  for (const transcript of transcripts) {
    const hintsJson = store.getCell(
      "transcripts",
      transcript.id,
      "speaker_hints",
    );
    if (typeof hintsJson !== "string") continue;

    try {
      const parsedHints = JSON.parse(hintsJson);
      storageHints.push(...parsedHints);
    } catch {
      continue;
    }
  }

  return convertStorageHintsToRuntime(storageHints, wordIdToIndex);
}

function toSegmentPayload(
  segment: SegmentForPayload,
  sessionStartMs: number,
  store: MainStore,
  speakerLabelManager: SpeakerLabelManager,
): SegmentPayload {
  const firstWord = segment.words[0];
  const lastWord = segment.words[segment.words.length - 1];

  const absoluteStartMs = firstWord.transcriptStartedAt + firstWord.start_ms;
  const absoluteEndMs = lastWord.transcriptStartedAt + lastWord.end_ms;

  const ctx = defaultRenderLabelContext(store);
  const label = SegmentKey.renderLabel(segment.key, ctx, speakerLabelManager);

  return {
    speaker_label: label,
    start_ms: absoluteStartMs - sessionStartMs,
    end_ms: absoluteEndMs - sessionStartMs,
    text: segment.words.map((word) => word.text).join(" "),
    words: segment.words.map((word) => ({
      text: word.text,
      start_ms: word.transcriptStartedAt + word.start_ms - sessionStartMs,
      end_ms: word.transcriptStartedAt + word.end_ms - sessionStartMs,
    })),
  };
}

function getStringCell(
  store: MainStore,
  tableId: any,
  rowId: string,
  columnId: string,
): string {
  const value = store.getCell(tableId, rowId, columnId);
  return typeof value === "string" ? value : "";
}

function getOptionalStringCell(
  store: MainStore,
  tableId: any,
  rowId: string,
  columnId: string,
): string | undefined {
  const value = store.getCell(tableId, rowId, columnId);
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function getNumberCell(
  store: MainStore,
  tableId: any,
  rowId: string,
  columnId: string,
): number | undefined {
  const value = store.getCell(tableId, rowId, columnId);
  return typeof value === "number" ? value : undefined;
}
