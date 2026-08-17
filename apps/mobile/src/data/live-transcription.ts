import { supabase } from "@/auth/client";
import { execute, executeTransaction } from "@/db";
import { captureAnalytics } from "@/lib/analytics";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { id, nowIso } from "@/lib/ids";

import {
  parseHostedTranscriptionMessage,
  type LiveSpeakerHint,
  type LiveTranscriptWord,
} from "./live-transcription-model";

export type LiveTranscriptionStatus = "connecting" | "live" | "fallback";

const MAX_QUEUED_AUDIO_BYTES = 320_000;
const MAX_SOCKET_BUFFERED_BYTES = 1_000_000;
const FINALIZE_TIMEOUT_MS = 3_000;
const KEEP_ALIVE_INTERVAL_MS = 5_000;

const TRANSCRIPT_TOMBSTONE_SQL = `
UPDATE transcripts
SET deleted_at = ?, updated_at = ?
WHERE session_id = ? AND deleted_at IS NULL
`;

const LIVE_TRANSCRIPT_INSERT_SQL = `
INSERT INTO transcripts (
  id, workspace_id, owner_user_id, session_id, source, provider,
  model, language, started_at_ms, ended_at_ms, audio_attachment_id,
  memo, words_json, speaker_hints_json, metadata_json, created_at,
  updated_at, deleted_at
)
SELECT ?, session.workspace_id, session.owner_user_id, session.id,
  'live_capture', 'anarlog', 'cloud', '', ?, NULL, ?,
  COALESCE((
    SELECT note.body FROM session_documents AS note
    WHERE note.id = session.id AND note.kind = 'note' AND note.deleted_at IS NULL
  ), ''),
  ?, ?, '{}', ?, ?, NULL
FROM sessions AS session
WHERE session.id = ? AND session.deleted_at IS NULL
`;

const TRANSCRIPT_FINISH_SQL = `
UPDATE transcripts
SET words_json = ?, speaker_hints_json = ?, ended_at_ms = ?,
  content_revision = content_revision + 1, updated_at = ?
WHERE id = ? AND deleted_at IS NULL
`;

const LIVE_STATE_INSERT_SQL = `
INSERT OR IGNORE INTO transcript_live_state (
  transcript_id, next_sequence, updated_at
)
SELECT id, 0, ?
FROM transcripts
WHERE id = ? AND deleted_at IS NULL
`;

const LIVE_DELTA_INSERT_SQL = `
INSERT INTO transcript_live_deltas (
  id, transcript_id, sequence, delta_json, created_at
)
SELECT ?, transcript_id, next_sequence, ?, ?
FROM transcript_live_state
WHERE transcript_id = ?
`;

const LIVE_STATE_ADVANCE_SQL = `
UPDATE transcript_live_state
SET next_sequence = next_sequence + 1, updated_at = ?
WHERE transcript_id = ?
`;

const LIVE_STATE_DELETE_SQL = `
DELETE FROM transcript_live_state
WHERE transcript_id = ?
`;

const MARK_AUDIO_COMPLETE_SQL = `
UPDATE session_attachments
SET metadata_json = json_set(
  CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
  '$.transcript_status', 'complete'
), updated_at = ?
WHERE id = ? AND session_id = ? AND source_type = 'session_audio'
  AND source_id = 'primary' AND deleted_at IS NULL
`;

type NativeWebSocketConstructor = new (
  url: string,
  protocols?: string[] | null,
  options?: { headers?: Record<string, string> },
) => WebSocket;

function liveUrl(sampleRate: number, channels: number): string {
  const url = new URL(`${env.apiUrl}/stt/listen`);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.searchParams.set("provider", "anarlog");
  url.searchParams.set("model", "cloud");
  url.searchParams.set("encoding", "linear16");
  url.searchParams.set("sample_rate", String(sampleRate));
  url.searchParams.set("channels", String(channels));
  return url.toString();
}

export class HostedLiveTranscription {
  readonly transcriptId = id();

  private socket: WebSocket | null = null;
  private status: LiveTranscriptionStatus = "connecting";
  private queuedAudio: ArrayBuffer[] = [];
  private queuedAudioBytes = 0;
  private wordsById = new Map<string, LiveTranscriptWord>();
  private hintsById = new Map<string, LiveSpeakerHint>();
  private segmentWordIds = new Map<string, string[]>();
  private inserted = false;
  private readonly startedAtMs = Date.now();
  private writeChain = Promise.resolve();
  private stopping = false;
  private stopPromise: Promise<boolean> | null = null;
  private connectPromise: Promise<void>;
  private keepAliveTimer: ReturnType<typeof setInterval> | null = null;
  private terminalResolve: (() => void) | null = null;
  private terminalPromise = new Promise<void>((resolve) => {
    this.terminalResolve = resolve;
  });

  constructor(
    private readonly sessionId: string,
    sampleRate: number,
    channels: number,
    private readonly onUpdate: (update: {
      status: LiveTranscriptionStatus;
      text: string;
    }) => void,
  ) {
    this.connectPromise = this.connect(sampleRate, channels);
  }

  private setStatus(status: LiveTranscriptionStatus, text = "") {
    this.status = status;
    this.onUpdate({ status, text });
  }

  private async connect(sampleRate: number, channels: number) {
    try {
      const auth = await supabase?.auth.getSession();
      if (auth?.error) throw auth.error;
      const token = auth?.data.session?.access_token;
      if (!token)
        throw new Error("Live transcription requires a signed-in session");
      if (this.stopping || this.status === "fallback") return;

      const Socket = WebSocket as unknown as NativeWebSocketConstructor;
      const socket = new Socket(liveUrl(sampleRate, channels), null, {
        headers: { Authorization: `Bearer ${token}` },
      });
      socket.binaryType = "arraybuffer";
      socket.onopen = () => {
        if (this.stopping || this.status === "fallback") {
          socket.close();
          return;
        }
        this.setStatus("live");
        for (const buffer of this.queuedAudio) socket.send(buffer);
        this.queuedAudio = [];
        this.queuedAudioBytes = 0;
        this.keepAliveTimer = setInterval(() => {
          if (socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ type: "KeepAlive" }));
          }
        }, KEEP_ALIVE_INTERVAL_MS);
      };
      socket.onmessage = (event) => {
        if (typeof event.data !== "string") return;
        this.handleMessage(event.data);
      };
      socket.onerror = () => {
        this.fail(new Error("Live transcription socket failed"), "socket");
      };
      socket.onclose = () => {
        this.clearKeepAlive();
        this.terminalResolve?.();
        if (!this.stopping && this.status !== "fallback") {
          this.fail(
            new Error("Live transcription socket closed unexpectedly"),
            "socket_closed",
          );
        }
      };
      this.socket = socket;
    } catch (error) {
      this.fail(error, "connect");
    }
  }

  sendAudio(buffer: ArrayBuffer) {
    if (this.status === "fallback" || this.stopping) return;
    const socket = this.socket;
    if (socket?.readyState === WebSocket.OPEN) {
      if (socket.bufferedAmount > MAX_SOCKET_BUFFERED_BYTES) {
        this.fail(
          new Error("Live transcription upload fell behind"),
          "backpressure",
        );
        return;
      }
      socket.send(buffer);
      return;
    }

    if (this.queuedAudioBytes + buffer.byteLength > MAX_QUEUED_AUDIO_BYTES) {
      this.fail(new Error("Live transcription connection timed out"), "queue");
      return;
    }
    const copy = buffer.slice(0);
    this.queuedAudio.push(copy);
    this.queuedAudioBytes += copy.byteLength;
  }

  private handleMessage(message: string) {
    for (const event of parseHostedTranscriptionMessage(
      message,
      this.transcriptId,
    )) {
      if (event.type === "error") {
        this.fail(new Error(event.message), "response");
        continue;
      }
      if (event.type === "terminal") {
        this.terminalResolve?.();
        continue;
      }
      if (event.type === "partial") {
        this.onUpdate({ status: this.status, text: event.text });
        continue;
      }

      const replacedIds = this.segmentWordIds.get(event.segmentId) ?? [];
      for (const wordId of replacedIds) {
        this.wordsById.delete(wordId);
        this.hintsById.delete(`${wordId}:speaker`);
      }
      this.segmentWordIds.set(
        event.segmentId,
        event.words.map((word) => word.id),
      );
      for (const word of event.words) this.wordsById.set(word.id, word);
      for (const hint of event.hints) this.hintsById.set(hint.id, hint);
      this.onUpdate({ status: this.status, text: event.text });
      this.persistDelta(event.words, replacedIds);
    }
  }

  private persistDelta(
    deltaWords: LiveTranscriptWord[],
    replacedIds: string[],
  ) {
    const words = [...this.wordsById.values()].sort(
      (a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms,
    );
    const hints = [...this.hintsById.values()];
    const deltaJson = JSON.stringify({
      new_words: deltaWords,
      replaced_ids: replacedIds,
      partials: [],
    });
    this.writeChain = this.writeChain
      .then(async () => {
        const now = nowIso();
        if (!this.inserted) {
          const [, inserted = 0, stateInserted = 0] = await executeTransaction([
            {
              sql: TRANSCRIPT_TOMBSTONE_SQL,
              params: [now, now, this.sessionId],
            },
            {
              sql: LIVE_TRANSCRIPT_INSERT_SQL,
              params: [
                this.transcriptId,
                this.startedAtMs,
                `session-audio:${this.sessionId}`,
                JSON.stringify(words),
                JSON.stringify(hints),
                now,
                now,
                this.sessionId,
              ],
            },
            {
              sql: LIVE_STATE_INSERT_SQL,
              params: [now, this.transcriptId],
            },
          ]);
          if (inserted !== 1 || stateInserted !== 1) {
            throw new Error("Live transcript could not be initialized");
          }
          this.inserted = true;
          return;
        }
        const [deltaInserted = 0, stateAdvanced = 0] = await executeTransaction(
          [
            {
              sql: LIVE_DELTA_INSERT_SQL,
              params: [id(), deltaJson, now, this.transcriptId],
            },
            {
              sql: LIVE_STATE_ADVANCE_SQL,
              params: [now, this.transcriptId],
            },
          ],
        );
        if (deltaInserted !== 1 || stateAdvanced !== 1) {
          throw new Error("Live transcript delta could not be saved");
        }
      })
      .then(() => {})
      .catch((error) => this.fail(error, "persist"));
  }

  private fail(error: unknown, stage: string) {
    if (this.status === "fallback") return;
    this.setStatus("fallback");
    this.clearKeepAlive();
    this.queuedAudio = [];
    this.queuedAudioBytes = 0;
    this.socket?.close();
    captureOperationalError(error, {
      operation: "transcription_live",
      tags: { mode: "live", stage },
    });
    captureAnalytics("transcription_failed", {
      mode: "live",
      entry_point: "mobile_audio",
      failure_stage: stage,
    });
  }

  stop(): Promise<boolean> {
    if (this.stopPromise) return this.stopPromise;
    this.stopPromise = this.performStop();
    return this.stopPromise;
  }

  private async performStop(): Promise<boolean> {
    this.stopping = true;
    await this.connectPromise;
    this.clearKeepAlive();
    const socket = this.socket;
    try {
      if (socket?.readyState === WebSocket.OPEN && this.status === "live") {
        socket.send(JSON.stringify({ type: "Finalize" }));
        let timer: ReturnType<typeof setTimeout>;
        await Promise.race([
          this.terminalPromise,
          new Promise<void>((resolve) => {
            timer = setTimeout(resolve, FINALIZE_TIMEOUT_MS);
          }),
        ]);
        clearTimeout(timer!);
        if (socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "CloseStream" }));
          socket.close();
        }
      } else {
        socket?.close();
      }
    } catch (error) {
      this.fail(error, "finalize");
      try {
        socket?.close();
      } catch {}
    }
    await this.writeChain;
    if (this.inserted) {
      const words = [...this.wordsById.values()].sort(
        (a, b) => a.start_ms - b.start_ms || a.end_ms - b.end_ms,
      );
      const hints = [...this.hintsById.values()];
      const [finished = 0] = await executeTransaction([
        {
          sql: TRANSCRIPT_FINISH_SQL,
          params: [
            JSON.stringify(words),
            JSON.stringify(hints),
            Date.now(),
            nowIso(),
            this.transcriptId,
          ],
        },
        {
          sql: LIVE_STATE_DELETE_SQL,
          params: [this.transcriptId],
        },
      ]).catch((error) => {
        this.fail(error, "finish");
        return [];
      });
      if (finished !== 1) {
        this.fail(
          new Error("Live transcript could not be finalized"),
          "finish",
        );
      }
    }
    const complete =
      this.status === "live" && this.inserted && this.wordsById.size > 0;
    if (complete) {
      captureAnalytics("transcription_completed", {
        mode: "live",
        entry_point: "mobile_audio",
      });
    }
    return complete;
  }

  private clearKeepAlive() {
    if (this.keepAliveTimer) clearInterval(this.keepAliveTimer);
    this.keepAliveTimer = null;
  }
}

export async function markSessionAudioTranscribed(
  sessionId: string,
): Promise<void> {
  await execute(MARK_AUDIO_COMPLETE_SQL, [
    nowIso(),
    `session-audio:${sessionId}`,
    sessionId,
  ]);
}
