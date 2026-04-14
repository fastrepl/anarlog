import type {
  Participant,
  Segment,
  Session,
  Transcript,
} from "@hypr/plugin-template";

import type { TaskArgsMap, TaskArgsMapTransformed, TaskConfig } from ".";

import { getSessionEventById } from "~/session/utils";
import type { Store as MainStore } from "~/store/tinybase/store/main";
import type { Store as SettingsStore } from "~/store/tinybase/store/settings";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";
import { getTemplateById } from "~/templates/queries";

type TranscriptMeta = {
  id: string;
  startedAt: number;
  endedAt: number | null;
  memoMd: string;
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
  const templateRecord = templateId ? await getTemplateById(templateId) : null;
  const template = templateRecord
    ? {
        title: templateRecord.title,
        description: templateRecord.description ?? null,
        sections: templateRecord.sections,
      }
    : null;
  const language = getLanguage(settingsStore);
  const segments = await getTranscriptSegmentsFromMeta(
    sessionContext.transcriptsMeta,
    store,
  );

  return {
    language,
    session: sessionContext.session,
    participants: sessionContext.participants,
    template,
    preMeetingMemo: sessionContext.preMeetingMemo,
    postMeetingMemo: sessionContext.postMeetingMemo,
    transcripts: formatTranscripts(segments, sessionContext.transcriptsMeta),
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

async function getTranscriptSegmentsFromMeta(
  transcripts: TranscriptMeta[],
  store: MainStore,
): Promise<SegmentPayload[]> {
  if (transcripts.length === 0) {
    return [];
  }

  const request = buildRenderTranscriptRequestFromStore(
    store,
    transcripts.map((transcript) => transcript.id),
  );
  if (!request) {
    return [];
  }

  const segments = await renderTranscriptSegments(request);

  const normalizedSegments = segments.reduce<SegmentPayload[]>(
    (acc, segment) => {
      if (segment.words.length === 0) {
        return acc;
      }

      acc.push(toSegmentPayload(segment));
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

function toSegmentPayload(
  segment: Awaited<ReturnType<typeof renderTranscriptSegments>>[number],
): SegmentPayload {
  return {
    speaker_label: segment.speaker_label,
    start_ms: segment.start_ms,
    end_ms: segment.end_ms,
    text: segment.text,
    words: segment.words.map((word) => ({
      text: word.text,
      start_ms: word.start_ms,
      end_ms: word.end_ms,
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
