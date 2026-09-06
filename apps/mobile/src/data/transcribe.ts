import { File, Paths } from "expo-file-system";
import { fetch } from "expo/fetch";
import { useSyncExternalStore } from "react";
import { Platform } from "react-native";

import { ProRequiredError } from "@/auth/billing";
import { execute, executeTransaction } from "@/db";
import { captureAnalytics } from "@/lib/analytics";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { id, nowIso } from "@/lib/ids";
import { readPreferences } from "@/settings/preferences";
import {
  applyTranscriptionPreferences,
  type Preferences,
} from "@/settings/preferences-model";
import { resolveProvider } from "@/settings/providers";
import {
  normalizeTranscriptionResponse,
  type ProviderConfig,
} from "@/settings/providers-model";

import { requestProviderTranscription } from "./provider-transcription";
import { TranscriptionAdmission } from "./transcription-admission";
import {
  assertBoundedTranscriptionResponse,
  boundedSyntheticTokens,
  MAX_TRANSCRIPTION_AUDIO_BYTES,
  MAX_TRANSCRIPTION_RESPONSE_BYTES,
  MAX_TRANSCRIPTION_WORDS,
  readBoundedTranscriptionResponse,
} from "./transcription-response";

// Client-driven batch STT, mirroring desktop's useRunBatch: POST the audio to
// the transcribe proxy, then persist the transcript exactly like desktop's
// createTranscript (source batch_transcription, whole-session replace).
export type TranscriptionState = "idle" | "running" | "failed";

const states = new Map<string, TranscriptionState>();
const inflight = new Map<string, Promise<void>>();
const automaticRetryBlocked = new Set<string>();
const listeners = new Set<() => void>();
const MAX_RETAINED_FAILED_STATES = 100;
const transcriptionAdmission = new TranscriptionAdmission(2, 32);

function setState(sessionId: string, state: TranscriptionState) {
  states.delete(sessionId);
  if (state !== "idle") {
    states.set(sessionId, state);
  }
  if (state === "failed") {
    let retainedFailures = [...states.values()].filter(
      (value) => value === "failed",
    ).length;
    for (const [retainedSessionId, retainedState] of states) {
      if (retainedFailures <= MAX_RETAINED_FAILED_STATES) {
        break;
      }
      if (retainedState === "failed") {
        states.delete(retainedSessionId);
        retainedFailures -= 1;
      }
    }
  }
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
SELECT
  attachment.content_type,
  attachment.size_bytes,
  local_state.relative_path AS local_relative_path
FROM session_attachments AS attachment
JOIN attachment_local_state AS local_state
  ON local_state.attachment_id = attachment.id
 AND local_state.availability = 'present'
WHERE attachment.session_id = ?
  AND attachment.source_type = 'session_audio'
  AND attachment.deleted_at IS NULL
  AND COALESCE(json_extract(attachment.metadata_json, '$.transcript_status'), '') <> 'complete'
LIMIT 1
`;

const PENDING_TRANSCRIPTION_RETRY_SQL = `
SELECT attachment.session_id
FROM session_attachments AS attachment
JOIN attachment_local_state AS local_state
  ON local_state.attachment_id = attachment.id
 AND local_state.availability = 'present'
WHERE attachment.source_type = 'session_audio'
  AND attachment.deleted_at IS NULL
  AND attachment.size_bytes > 0
  AND attachment.size_bytes <= ?
  AND COALESCE(json_extract(attachment.metadata_json, '$.transcript_status'), '') <> 'complete'
ORDER BY attachment.updated_at, attachment.id
LIMIT 8
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
  'batch_transcription', ?, ?, ?, ?, NULL, '',
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

function syntheticWords(
  transcript: string,
  channel: number,
  remainingWords: number,
): WordRecord[] {
  const tokens = boundedSyntheticTokens(transcript, remainingWords);
  const durationMs = tokens.length * SYNTHETIC_TEXT_WORD_MS;
  return tokens.map((token, index) => ({
    id: id(),
    text: token,
    start_ms: Math.round((index / tokens.length) * durationMs),
    end_ms: Math.round(((index + 1) / tokens.length) * durationMs),
    channel,
  }));
}

function mapBatchResponse(
  payload: unknown,
  provider: string,
): {
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
      if (words.length >= MAX_TRANSCRIPTION_WORDS) {
        throw transcriptionFailure(
          "STT response has too many words",
          "response",
          { code: "stt_response_too_large" },
        );
      }
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
            provider,
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
      words.push(
        ...syntheticWords(
          alternative.transcript,
          channelIndex,
          MAX_TRANSCRIPTION_WORDS - words.length,
        ),
      );
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
type TranscriptionStage =
  | "auth"
  | "load_audio"
  | "persist"
  | "request"
  | "response";

function transcriptionFailure(
  message: string,
  stage: TranscriptionStage,
  details?: { code?: string; status?: number },
) {
  return Object.assign(new Error(message), { stage, ...details });
}

function withTranscriptionStage(
  error: unknown,
  stage: TranscriptionStage,
): unknown {
  if (error && typeof error === "object") {
    try {
      Object.defineProperty(error, "stage", {
        configurable: true,
        value: stage,
      });
      return error;
    } catch {}
  }
  return transcriptionFailure("Transcription failed", stage);
}

function transcriptionStage(error: unknown): TranscriptionStage | "unknown" {
  if (!error || typeof error !== "object") return "unknown";
  const stage = (error as { stage?: unknown }).stage;
  return ["auth", "load_audio", "persist", "request", "response"].includes(
    String(stage),
  )
    ? (stage as TranscriptionStage)
    : "unknown";
}

function isPermanentTranscriptionFailure(error: unknown): boolean {
  if (!error || typeof error !== "object") return false;
  const code = (error as { code?: unknown }).code;
  return [
    "audio_missing",
    "audio_too_large",
    "stt_response_too_large",
  ].includes(String(code));
}

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
// iOS background session survives the 60s idle timeout). Web passes the Blob
// directly so the browser can stream it without first materializing its bytes.
async function requestTranscription(
  file: File,
  contentType: string,
  token: string | undefined,
  timeoutMs: number,
  provider: ProviderConfig & { apiKey: string },
  preferences: Preferences,
): Promise<{ status: number; body: string }> {
  const url = applyTranscriptionPreferences(
    new URL(
      `${env.apiUrl}/stt/listen?provider=anarlog&max_response_bytes=${MAX_TRANSCRIPTION_RESPONSE_BYTES}`,
    ),
    preferences,
  ).toString();
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
    if (provider.provider !== "anarlog")
      return await requestProviderTranscription(
        file,
        contentType,
        provider,
        preferences,
        controller.signal,
      );
    if (Platform.OS === "web") {
      const response = await fetch(url, {
        method: "POST",
        headers,
        body: file,
        signal: controller.signal,
      });
      return {
        status: response.status,
        body: await readBoundedTranscriptionResponse(response),
      };
    }
    // Expo's native upload API streams the request from disk but returns the
    // response as one materialized string and exposes no response-size option.
    // The proxy cap is therefore the hard native boundary; this assertion also
    // rejects a misconfigured or older proxy after it returns.
    const result = await file.upload(url, {
      httpMethod: "POST",
      headers,
      signal: controller.signal,
    });
    const contentLength =
      Object.entries(result.headers).find(
        ([name]) => name.toLowerCase() === "content-length",
      )?.[1] ?? null;
    assertBoundedTranscriptionResponse(result.body, contentLength);
    return { status: result.status, body: result.body };
  } finally {
    clearTimeout(timer);
  }
}

async function runTranscription(sessionId: string): Promise<void> {
  const rows = await execute<{
    content_type: string;
    size_bytes: number;
    local_relative_path: string;
  }>(PENDING_AUDIO_SQL, [sessionId]);
  const audio = rows[0];
  if (!audio) return;

  const file = new File(
    Paths.document,
    "sessions",
    sessionId,
    audio.local_relative_path,
  );
  if (!file.exists) {
    throw transcriptionFailure("Audio file missing", "load_audio", {
      code: "audio_missing",
    });
  }
  const sizeBytes = Math.max(audio.size_bytes, file.size);
  if (sizeBytes > MAX_TRANSCRIPTION_AUDIO_BYTES) {
    throw transcriptionFailure("Audio file is too large", "load_audio", {
      code: "audio_too_large",
    });
  }

  const provider = await resolveProvider("stt");
  const preferences = await readPreferences();
  const token = provider.provider === "anarlog" ? provider.apiKey : undefined;
  const startedAtMs = Date.now();

  const response = await requestTranscription(
    file,
    audio.content_type || "application/octet-stream",
    token,
    requestTimeoutMs(sizeBytes),
    provider,
    preferences,
  ).catch((error) => {
    throw withTranscriptionStage(error, "request");
  });
  if (response.status < 200 || response.status >= 300) {
    if (response.status === 413) {
      throw transcriptionFailure("STT response is too large", "response", {
        code: "stt_response_too_large",
        status: response.status,
      });
    }
    throw transcriptionFailure("STT request failed", "response", {
      code: "stt_http_error",
      status: response.status,
    });
  }

  let payload: unknown;
  try {
    payload = normalizeTranscriptionResponse(
      provider.provider,
      JSON.parse(response.body),
    );
  } catch (error) {
    throw withTranscriptionStage(error, "response");
  }
  response.body = "";
  const { words, hints } = mapBatchResponse(payload, provider.provider);
  payload = null;
  if (words.length === 0) {
    // Never mark complete on an empty result — transcript_status stays
    // 'processing' so the tap-to-retry affordance remains reachable.
    throw transcriptionFailure("STT returned no words", "response", {
      code: "stt_empty_result",
    });
  }
  const now = nowIso();
  const attachmentId = `session-audio:${sessionId}`;
  const wordsJson = JSON.stringify(words);
  words.length = 0;
  const hintsJson = JSON.stringify(hints);
  hints.length = 0;
  await executeTransaction([
    {
      sql: TRANSCRIPT_TOMBSTONE_SQL,
      params: [now, now, sessionId] as unknown[],
    },
    {
      sql: TRANSCRIPT_INSERT_SQL,
      params: [
        id(),
        provider.provider,
        provider.model,
        preferences.ai_language,
        startedAtMs,
        wordsJson,
        hintsJson,
        now,
        now,
        sessionId,
      ] as unknown[],
    },
    {
      sql: MARK_COMPLETE_SQL,
      params: [now, attachmentId, sessionId] as unknown[],
    },
  ]).catch((error) => {
    throw withTranscriptionStage(error, "persist");
  });
}

export function transcribeSession(sessionId: string): Promise<void> {
  const existing = inflight.get(sessionId);
  if (existing) return existing;

  const admitted = transcriptionAdmission.schedule(() =>
    runTranscription(sessionId)
      .then(() => {
        captureAnalytics("transcription_completed", {
          mode: "batch",
          entry_point: "mobile_audio",
        });
        automaticRetryBlocked.delete(sessionId);
        clearStateSilently(sessionId);
      })
      .catch((error: unknown) => {
        if (error instanceof ProRequiredError) {
          setState(sessionId, "idle");
          return;
        }
        captureOperationalError(error, {
          operation: "transcription_batch",
          tags: {
            mode: "batch",
            stage: transcriptionStage(error),
          },
        });
        captureAnalytics("transcription_failed", {
          mode: "batch",
          entry_point: "mobile_audio",
          failure_stage: "transcription",
        });
        if (isPermanentTranscriptionFailure(error)) {
          automaticRetryBlocked.add(sessionId);
        }
        setState(sessionId, "failed");
      }),
  );
  if (!admitted) {
    const error = transcriptionFailure(
      "Too many transcriptions are already queued",
      "request",
      { code: "stt_admission_full" },
    );
    captureOperationalError(error, {
      operation: "transcription_batch",
      tags: { mode: "batch", stage: "request" },
    });
    setState(sessionId, "failed");
    return Promise.resolve();
  }

  setState(sessionId, "running");
  const task = admitted.finally(() => {
    inflight.delete(sessionId);
  });
  inflight.set(sessionId, task);
  return task;
}

let pendingRetryPass: Promise<void> | null = null;

export function retryPendingTranscriptions(): Promise<void> {
  if (pendingRetryPass) return pendingRetryPass;

  const pass = execute<{ session_id: string }>(
    PENDING_TRANSCRIPTION_RETRY_SQL,
    [MAX_TRANSCRIPTION_AUDIO_BYTES],
  )
    .then((rows) =>
      Promise.all(
        rows
          .map((row) => row.session_id)
          .filter(
            (sessionId) =>
              !automaticRetryBlocked.has(sessionId) && !inflight.has(sessionId),
          )
          .map((sessionId) => transcribeSession(sessionId)),
      ),
    )
    .then(() => {})
    .catch((error) => {
      captureOperationalError(error, {
        operation: "transcription_retry_pending",
        level: "warning",
      });
    });
  pendingRetryPass = pass.finally(() => {
    pendingRetryPass = null;
  });
  return pendingRetryPass;
}
