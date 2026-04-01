import type {
  SessionContentData,
  TranscriptSpeakerHint,
} from "@hypr/plugin-fs-sync";
import { commands as listenerCommands } from "@hypr/plugin-listener";
import type {
  IdentityAssignment,
  RenderTranscriptHuman,
  RenderTranscriptInput,
  RenderTranscriptRequest,
  RenderedTranscriptSegment,
} from "@hypr/plugin-listener";

import type * as main from "~/store/tinybase/store/main";
import { parseTranscriptHints, parseTranscriptWords } from "~/stt/utils";

type TranscriptRow = {
  started_at?: number | null;
  words?: Array<{
    id?: string | null;
    text?: string | null;
    start_ms?: number | null;
    end_ms?: number | null;
    channel?: number | null;
  }> | null;
  speaker_hints?: Array<
    TranscriptSpeakerHint | { word_id?: string; type?: string; value?: unknown }
  > | null;
};

type RenderTranscriptRequestHumans = {
  selfHumanId?: string;
  humans: RenderTranscriptHuman[];
};

export async function renderTranscriptSegments(
  request: RenderTranscriptRequest,
): Promise<RenderedTranscriptSegment[]> {
  const result = await listenerCommands.renderTranscriptSegments(
    normalizeRenderTranscriptRequest(request),
  );
  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}

export function buildRenderTranscriptRequestFromStore(
  store: NonNullable<ReturnType<typeof main.UI.useStore>>,
  transcriptIds: string[],
): RenderTranscriptRequest | null {
  const sessionId = getSessionIdForTranscripts(store, transcriptIds);
  const transcripts = transcriptIds.map((transcriptId) => ({
    started_at: asNumber(
      store.getCell("transcripts", transcriptId, "started_at"),
    ),
    words: parseTranscriptWords(store, transcriptId),
    speaker_hints: parseTranscriptHints(store, transcriptId),
  }));

  return buildRenderTranscriptRequest(
    transcripts,
    collectRenderHumans(store),
    collectSessionParticipantHumanIds(store, sessionId),
  );
}

export function buildRenderTranscriptRequestFromFsTranscript(
  transcriptData: SessionContentData["transcript"],
  store?: ReturnType<typeof main.UI.useStore>,
  sessionId?: string,
): RenderTranscriptRequest | null {
  return buildRenderTranscriptRequest(
    transcriptData?.transcripts ?? [],
    store ? collectRenderHumans(store) : undefined,
    store ? collectSessionParticipantHumanIds(store, sessionId) : undefined,
  );
}

function buildRenderTranscriptRequest(
  transcripts: TranscriptRow[],
  humans?: RenderTranscriptRequestHumans,
  participantHumanIds?: string[],
): RenderTranscriptRequest | null {
  if (transcripts.length === 0) {
    return null;
  }

  const normalizedTranscripts: RenderTranscriptInput[] = [];

  for (const transcript of transcripts) {
    const words: RenderTranscriptInput["words"] = [];
    const assignments: IdentityAssignment[] = [];
    const wordIndexById = new Map<string, number>();

    for (const word of transcript.words ?? []) {
      if (
        typeof word.id !== "string" ||
        typeof word.text !== "string" ||
        typeof word.start_ms !== "number" ||
        typeof word.end_ms !== "number"
      ) {
        continue;
      }

      wordIndexById.set(word.id, words.length);
      words.push({
        id: word.id,
        text: word.text,
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        channel: typeof word.channel === "number" ? word.channel : 0,
        speaker_index: null,
      });
    }

    for (const hint of transcript.speaker_hints ?? []) {
      const normalized = normalizeSpeakerHint(hint, words, wordIndexById);
      if (normalized) {
        assignments.push(normalized);
      }
    }

    if (words.length === 0) {
      continue;
    }

    normalizedTranscripts.push({
      started_at:
        typeof transcript.started_at === "number"
          ? transcript.started_at
          : null,
      words,
      assignments,
    });
  }

  if (normalizedTranscripts.length === 0) {
    return null;
  }

  return {
    transcripts: normalizedTranscripts,
    participant_human_ids: participantHumanIds ?? [],
    self_human_id: humans?.selfHumanId ?? null,
    humans: humans?.humans ?? [],
  };
}

function normalizeSpeakerHint(
  hint:
    | TranscriptSpeakerHint
    | { word_id?: string; type?: string; value?: unknown },
  words: RenderTranscriptInput["words"],
  wordIndexById: Map<string, number>,
): IdentityAssignment | null {
  if (typeof hint.word_id !== "string" || typeof hint.type !== "string") {
    return null;
  }

  const value = parseHintValue(hint.value);
  if (!value || typeof value !== "object") {
    return null;
  }

  const wordIndex = wordIndexById.get(hint.word_id);
  if (typeof wordIndex !== "number") {
    return null;
  }

  const word = words[wordIndex];
  if (!word) {
    return null;
  }

  if (
    hint.type === "provider_speaker_index" &&
    typeof (value as { speaker_index?: unknown }).speaker_index === "number"
  ) {
    word.speaker_index = (value as { speaker_index: number }).speaker_index;
    if (typeof (value as { channel?: unknown }).channel === "number") {
      word.channel = (value as { channel: number }).channel;
    }
    return null;
  }

  if (
    hint.type === "user_speaker_assignment" &&
    typeof (value as { human_id?: unknown }).human_id === "string"
  ) {
    const humanId = (value as { human_id: string }).human_id;
    return word.speaker_index == null
      ? {
          human_id: humanId,
          scope: {
            kind: "channel",
            channel:
              word.channel === 0
                ? "DirectMic"
                : word.channel === 1
                  ? "RemoteParty"
                  : "MixedCapture",
          },
        }
      : {
          human_id: humanId,
          scope: {
            kind: "channel_speaker",
            channel:
              word.channel === 0
                ? "DirectMic"
                : word.channel === 1
                  ? "RemoteParty"
                  : "MixedCapture",
            speaker_index: word.speaker_index,
          },
        };
  }

  return null;
}

function parseHintValue(value: unknown): unknown {
  if (typeof value === "string") {
    try {
      return JSON.parse(value);
    } catch {
      return undefined;
    }
  }

  return value;
}

function collectRenderHumans(
  store: Pick<main.Store, "forEachRow" | "getValue" | "getRow">,
): RenderTranscriptRequestHumans {
  const humans: RenderTranscriptHuman[] = [];

  store.forEachRow("humans", (humanId, _forEachCell) => {
    const row = store.getRow("humans", humanId);
    if (typeof row.name !== "string" || !row.name) {
      return;
    }

    humans.push({
      human_id: humanId,
      name: row.name,
    });
  });

  const selfHumanId = store.getValue("user_id");

  return {
    selfHumanId: typeof selfHumanId === "string" ? selfHumanId : undefined,
    humans,
  };
}

function getSessionIdForTranscripts(
  store: Pick<main.Store, "getCell">,
  transcriptIds: string[],
): string | undefined {
  for (const transcriptId of transcriptIds) {
    const sessionId = store.getCell("transcripts", transcriptId, "session_id");
    if (typeof sessionId === "string" && sessionId) {
      return sessionId;
    }
  }

  return undefined;
}

function collectSessionParticipantHumanIds(
  store: Pick<main.Store, "forEachRow" | "getCell">,
  sessionId?: string,
): string[] {
  if (!sessionId) {
    return [];
  }

  const participantHumanIds: string[] = [];
  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mappingSessionId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "session_id",
    );
    if (mappingSessionId !== sessionId) {
      return;
    }

    const humanId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "human_id",
    );
    if (typeof humanId === "string" && humanId) {
      participantHumanIds.push(humanId);
    }
  });

  return participantHumanIds;
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" ? value : null;
}

function normalizeRenderTranscriptRequest(
  request: RenderTranscriptRequest,
): RenderTranscriptRequest {
  return {
    ...request,
    transcripts: request.transcripts.map((transcript) => ({
      ...transcript,
      started_at: normalizeOptionalTranscriptMs(transcript.started_at),
      words: transcript.words.map((word) => ({
        ...word,
        start_ms: normalizeTranscriptMs(word.start_ms),
        end_ms: normalizeTranscriptMs(word.end_ms),
      })),
    })),
  };
}

function normalizeTranscriptMs(value: number): number {
  return Number.isFinite(value) ? Math.round(value) : value;
}

function normalizeOptionalTranscriptMs(value: number | null): number | null {
  return typeof value === "number" ? normalizeTranscriptMs(value) : value;
}
