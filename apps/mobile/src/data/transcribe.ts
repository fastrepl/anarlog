import { File, Paths } from "expo-file-system";
import { fetch } from "expo/fetch";
import { useSyncExternalStore } from "react";
import { Platform } from "react-native";

import { supabase } from "@/auth/client";
import { execute, executeTransaction } from "@/db";
import { env } from "@/lib/env";
import { id, nowIso } from "@/lib/ids";

// Client-driven batch STT, mirroring desktop's useRunBatch: POST the audio to
// the transcribe proxy, then persist the transcript exactly like desktop's
// createTranscript (source batch_transcription, whole-session replace).
export type TranscriptionState = "idle" | "running" | "failed";

const states = new Map<string, TranscriptionState>();
const inflight = new Map<string, Promise<void>>();
const listeners = new Set<() => void>();

function setState(sessionId: string, state: TranscriptionState) {
  if (state === "idle") states.delete(sessionId);
  else states.set(sessionId, state);
  for (const listener of listeners) listener();
}

// On success the committed transcript rows re-render the note screen via the
// live query; clearing without notifying avoids a one-frame "Tap to
// transcribe" flash between the idle notification and the query refresh.
function clearStateSilently(sessionId: string) {
  states.delete(sessionId);
}

export function useTranscriptionState(sessionId: string): TranscriptionState {
  return useSyncExternalStore(
    (onChange) => {
      listeners.add(onChange);
      return () => listeners.delete(onChange);
    },
    () => states.get(sessionId) ?? "idle",
  );
}

const PENDING_AUDIO_SQL = `
SELECT filename, content_type, size_bytes
FROM session_attachments
WHERE session_id = ? AND source_type = 'session_audio' AND deleted_at IS NULL
  AND COALESCE(json_extract(metadata_json, '$.transcript_status'), '') <> 'complete'
LIMIT 1
`;

const TRANSCRIPT_TOMBSTONE_SQL = `
UPDATE transcripts
SET deleted_at = ?, updated_at = ?
WHERE session_id = ? AND deleted_at IS NULL
`;

const TRANSCRIPT_INSERT_SQL = `
INSERT INTO transcripts (
  id, workspace_id, owner_user_id, session_id, source, provider,
  model, language, started_at_ms, ended_at_ms, audio_attachment_id,
  memo, words_json, speaker_hints_json, metadata_json, created_at,
  updated_at, deleted_at
)
SELECT ?, session.workspace_id, session.owner_user_id, session.id,
  'batch_transcription', 'hyprnote', 'cloud', ?, ?, NULL, '',
  COALESCE((
    SELECT note.body FROM session_documents AS note
    WHERE note.id = session.id AND note.kind = 'note' AND note.deleted_at IS NULL
  ), ''),
  ?, ?, '{}', ?, ?, NULL
FROM sessions AS session
WHERE session.id = ? AND session.deleted_at IS NULL
`;

const MARK_COMPLETE_SQL = `
UPDATE session_attachments
SET
  metadata_json = json_set(
    CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
    '$.transcript_status',
    'complete'
  ),
  updated_at = ?
WHERE id = ? AND session_id = ? AND source_type = 'session_audio'
  AND source_id = 'primary' AND deleted_at IS NULL
`;

type BatchWord = {
  word?: unknown;
  start?: unknown;
  end?: unknown;
  channel?: unknown;
  speaker?: unknown;
  punctuated_word?: unknown;
};

type WordRecord = {
  id: string;
  text: string;
  start_ms: number;
  end_ms: number;
  channel: number;
};

type HintRecord = {
  id: string;
  word_id: string;
  type: "provider_speaker_index";
  value: string;
};

// Desktop's synthetic_text fallback (batch.ts wordEntriesFromTranscript):
// providers like OpenAI/Mistral may return a transcript with no word timings.
const SYNTHETIC_TEXT_WORD_MS = 400;

function syntheticWords(transcript: string, channel: number): WordRecord[] {
  const tokens = transcript.trim().split(/\s+/).filter(Boolean);
  const durationMs = tokens.length * SYNTHETIC_TEXT_WORD_MS;
  return tokens.map((token, index) => ({
    id: id(),
    text: token,
    start_ms: Math.round((index / tokens.length) * durationMs),
    end_ms: Math.round(((index + 1) / tokens.length) * durationMs),
    channel,
  }));
}

function mapBatchResponse(payload: unknown): {
  words: WordRecord[];
  hints: HintRecord[];
} {
  const channels =
    (
      payload as {
        results?: {
          channels?: {
            alternatives?: { transcript?: unknown; words?: BatchWord[] }[];
          }[];
        };
      }
    )?.results?.channels ?? [];

  const words: WordRecord[] = [];
  const hints: HintRecord[] = [];
  channels.forEach((channel, channelIndex) => {
    const alternative = channel?.alternatives?.[0];
    const channelWordCount = words.length;
    for (const word of alternative?.words ?? []) {
      if (typeof word?.word !== "string" || word.word === "") continue;
      const wordId = id();
      const wordChannel =
        typeof word.channel === "number" ? word.channel : channelIndex;
      words.push({
        id: wordId,
        text:
          typeof word.punctuated_word === "string" && word.punctuated_word
            ? word.punctuated_word
            : word.word,
        start_ms: Math.round(
          (typeof word.start === "number" ? word.start : 0) * 1000,
        ),
        end_ms: Math.round(
          (typeof word.end === "number" ? word.end : 0) * 1000,
        ),
        channel: wordChannel,
      });
      if (typeof word.speaker === "number") {
        hints.push({
          id: id(),
          word_id: wordId,
          type: "provider_speaker_index",
          value: JSON.stringify({
            provider: "hyprnote",
            channel: wordChannel,
            speaker_index: word.speaker,
          }),
        });
      }
    }
    if (
      words.length === channelWordCount &&
      typeof alternative?.transcript === "string" &&
      alternative.transcript.trim() !== ""
    ) {
      words.push(...syntheticWords(alternative.transcript, channelIndex));
    }
  });
  return { words, hints };
}

const REQUEST_TIMEOUT_BASE_MS = 60_000;
const REQUEST_TIMEOUT_MAX_MS = 900_000;
// Upload on a slow link, plus the provider transcribing the same audio before
// the synchronous response comes back.
const UPLOAD_MS_PER_KIB = 8;
const PROCESSING_MS_PER_KIB = 8;

function requestTimeoutMs(sizeBytes: number): number {
  // The abort covers the whole /stt/listen round trip, not just the upload, so
  // budgeting for transfer alone aborts long recordings the provider is still
  // transcribing and reports them as failures.
  const kib = sizeBytes / 1024;
  return Math.min(
    REQUEST_TIMEOUT_BASE_MS +
      Math.round(kib * (UPLOAD_MS_PER_KIB + PROCESSING_MS_PER_KIB)),
    REQUEST_TIMEOUT_MAX_MS,
  );
}

// Native uses File.upload (streams from disk, honors explicit Content-Type,
// iOS background session survives the 60s idle timeout); expo/fetch there
// would buffer the file in memory and override Content-Type with the
// extension-inferred MIME. Web keeps fetch with a bytes body (no override).
async function requestTranscription(
  file: File,
  contentType: string,
  token: string | undefined,
  timeoutMs: number,
): Promise<{ status: number; body: string }> {
  const url = `${env.apiUrl}/stt/listen?provider=hyprnote`;
  const headers = {
    "Content-Type": contentType,
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };
  const controller = new AbortController();
  const timer = setTimeout(
    () => controller.abort(new Error("stt timeout")),
    timeoutMs,
  );
  try {
    if (Platform.OS === "web") {
      const response = await fetch(url, {
        method: "POST",
        headers,
        body: new Uint8Array(await file.arrayBuffer()),
        signal: controller.signal,
      });
      return { status: response.status, body: await response.text() };
    }
    const result = await file.upload(url, {
      httpMethod: "POST",
      headers,
      signal: controller.signal,
    });
    return { status: result.status, body: result.body };
  } finally {
    clearTimeout(timer);
  }
}

async function runTranscription(sessionId: string): Promise<void> {
  const rows = await execute<{
    filename: string;
    content_type: string;
    size_bytes: number;
  }>(PENDING_AUDIO_SQL, [sessionId]);
  const audio = rows[0];
  if (!audio) return;

  const file = new File(Paths.document, "sessions", sessionId, audio.filename);
  if (!file.exists) {
    throw new Error(`audio file missing: ${audio.filename}`);
  }

  const auth = await supabase?.auth.getSession();
  const token = auth?.data.session?.access_token;
  const startedAtMs = Date.now();

  const response = await requestTranscription(
    file,
    audio.content_type || "application/octet-stream",
    token,
    requestTimeoutMs(audio.size_bytes),
  );
  if (response.status < 200 || response.status >= 300) {
    throw new Error(
      `stt request failed: ${response.status} ${response.body.slice(0, 256)}`,
    );
  }

  const { words, hints } = mapBatchResponse(JSON.parse(response.body));
  if (words.length === 0) {
    // Never mark complete on an empty result — transcript_status stays
    // 'processing' so the tap-to-retry affordance remains reachable.
    throw new Error("stt returned no words");
  }
  const now = nowIso();
  const attachmentId = `session-audio:${sessionId}`;
  await executeTransaction([
    {
      sql: TRANSCRIPT_TOMBSTONE_SQL,
      params: [now, now, sessionId] as unknown[],
    },
    {
      sql: TRANSCRIPT_INSERT_SQL,
      params: [
        id(),
        "",
        startedAtMs,
        JSON.stringify(words),
        JSON.stringify(hints),
        now,
        now,
        sessionId,
      ] as unknown[],
    },
    {
      sql: MARK_COMPLETE_SQL,
      params: [now, attachmentId, sessionId] as unknown[],
    },
  ]);
}

export function transcribeSession(sessionId: string): Promise<void> {
  const existing = inflight.get(sessionId);
  if (existing) return existing;

  setState(sessionId, "running");
  const task = runTranscription(sessionId)
    .then(() => clearStateSilently(sessionId))
    .catch((error: unknown) => {
      console.warn("[transcribe] failed", error);
      setState(sessionId, "failed");
    })
    .finally(() => {
      inflight.delete(sessionId);
    });
  inflight.set(sessionId, task);
  return task;
}
