import { useMutationState } from "@tanstack/react-query";
import { fetch } from "expo/fetch";

import { execute, executeTransaction } from "@/db";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { id, nowIso } from "@/lib/ids";
import { queryClient } from "@/lib/query-client";
import { readPreferences } from "@/settings/preferences";
import { resolveProvider } from "@/settings/providers";

import { docToPlainText, stripMarkdownTitle } from "./note-doc";
import { summaryRequest } from "./provider-summary";
import { buildSummaryPrompt, readSummaryText } from "./summary-model";
import {
  SESSION_TRANSCRIPTS_SQL,
  SESSION_SPEAKERS_SQL,
  transcriptSegments,
  type TranscriptRow,
} from "./transcript-model";
import { readBoundedTranscriptionResponse } from "./transcription-response";

async function runSummary(
  sessionId: string,
  automatic: boolean,
): Promise<void> {
  const existing = await execute<{ id: string; updated_at: string }>(
    "SELECT id, updated_at FROM session_documents WHERE session_id = ? AND kind = 'summary' AND deleted_at IS NULL ORDER BY sort_order, created_at, id LIMIT 1",
    [sessionId],
  );
  if (automatic && existing.length > 0) return;
  const [notes, transcripts, humans, preferences, provider] = await Promise.all(
    [
      execute<{ body: string; body_format: string }>(
        "SELECT body, body_format FROM session_documents WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL ORDER BY CASE WHEN id = session_id THEN 0 ELSE 1 END, sort_order, id LIMIT 1",
        [sessionId],
      ),
      execute<TranscriptRow>(SESSION_TRANSCRIPTS_SQL, [sessionId]),
      execute<{ id: string; name: string }>(SESSION_SPEAKERS_SQL, [sessionId]),
      readPreferences(),
      resolveProvider("llm"),
    ],
  );
  const note = notes[0];
  const text = note
    ? (note.body_format === "markdown"
        ? stripMarkdownTitle(note.body)
        : docToPlainText(note.body)
      ).text
    : "";
  const names = new Map(humans.map((human) => [human.id, human.name]));
  const transcript = transcripts
    .flatMap((row) => transcriptSegments(row, names))
    .map((segment) => `${segment.speaker}: ${segment.text}`)
    .join("\n");
  const source = `Notes:\n${text}\n\nTranscript:\n${transcript}`;
  if (!text.trim() && !transcript.trim())
    throw new Error("Add notes or transcribe a recording first.");
  if (source.length > 200_000)
    throw new Error(
      "This meeting is too long to summarize on mobile. Open it on desktop.",
    );
  const request = summaryRequest(
    provider,
    buildSummaryPrompt(preferences),
    source,
    env.apiUrl,
  );
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 120_000);
  let summary: string;
  try {
    const response = await fetch(request.url, {
      method: "POST",
      signal: controller.signal,
      redirect: "error",
      headers: request.headers,
      body: JSON.stringify(request.body),
    });
    if (!response.ok)
      throw new Error(
        response.status === 401 || response.status === 403
          ? "Check your provider API key or sign in again."
          : `The summary provider could not complete the request (${response.status}).`,
      );
    summary = readSummaryText(
      provider.provider,
      JSON.parse(await readBoundedTranscriptionResponse(response, 1024 * 1024)),
    );
  } finally {
    clearTimeout(timeout);
  }
  const now = nowIso();
  const metadata = JSON.stringify({
    provider: provider.provider,
    model: provider.model,
    language: preferences.ai_language,
    summary_length: preferences.summary_length,
  });
  const prior = existing[0];
  const [changed] = await executeTransaction([
    prior
      ? {
          sql: `UPDATE session_documents SET body = ?, body_format = 'markdown', generation_metadata_json = ?, updated_at = ?
      WHERE id = ? AND session_id = ? AND updated_at = ? AND deleted_at IS NULL
        AND EXISTS (SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)`,
          params: [
            summary,
            metadata,
            now,
            prior.id,
            sessionId,
            prior.updated_at,
            sessionId,
          ],
        }
      : {
          sql: `INSERT INTO session_documents (id, workspace_id, session_id, kind, title, body_format, body, generation_metadata_json, created_at, updated_at)
      SELECT ?, workspace_id, id, 'summary', 'Summary', 'markdown', ?, ?, ?, ? FROM sessions WHERE id = ? AND deleted_at IS NULL
        AND NOT EXISTS (SELECT 1 FROM session_documents WHERE session_id = ? AND kind = 'summary' AND deleted_at IS NULL)`,
          params: [id(), summary, metadata, now, now, sessionId, sessionId],
        },
  ]);
  if (changed !== 1)
    throw new Error(
      "This note changed while generating its summary. Please try again.",
    );
}

const inflight = new Map<string, Promise<void>>();

export function summarizeSession(
  sessionId: string,
  {
    automatic = false,
    beforeGenerate,
  }: {
    automatic?: boolean;
    beforeGenerate?: () => void | Promise<void>;
  } = {},
): Promise<void> {
  const pending = inflight.get(sessionId);
  if (pending) return pending;
  const mutation = queryClient.getMutationCache().build(queryClient, {
    mutationKey: ["session-summary", sessionId],
    mutationFn: async () => {
      await beforeGenerate?.();
      await runSummary(sessionId, automatic);
    },
    retry: false,
    onError: (error) =>
      captureOperationalError(error, { operation: "session_summary" }),
  });
  const promise = mutation
    .execute(undefined)
    .finally(() => inflight.delete(sessionId));
  inflight.set(sessionId, promise);
  return promise;
}

export function generateSummaryAfterTranscription(sessionId: string): void {
  // Summary failures are visible in the note; they must never fail audio persistence.
  void summarizeSession(sessionId, { automatic: true }).catch(() => {});
}

export function useSessionSummaryState(sessionId: string) {
  const states = useMutationState({
    filters: { mutationKey: ["session-summary", sessionId], exact: true },
    select: (mutation) => ({
      status: mutation.state.status,
      error: mutation.state.error,
    }),
  });
  return states.at(-1);
}
