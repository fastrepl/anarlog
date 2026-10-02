import {
  queryOptions,
  useMutationState,
  useQuery,
} from "@tanstack/react-query";
import { fetch } from "expo/fetch";

import { hasSummaryContent } from "@anlg/utils/session";
import { getSummaryEligibility } from "@anlg/utils/summary-eligibility";

import { execute, executeTransaction } from "@/db";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { id, nowIso } from "@/lib/ids";
import { queryClient } from "@/lib/query-client";
import { showToast } from "@/lib/toast";
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

export class SummarySkippedError extends Error {
  readonly reason: string;

  constructor(reason: string) {
    super(`Summary wasn't generated. ${reason}.`);
    this.reason = reason;
    this.name = "SummarySkippedError";
  }
}

export function notifySummarySkipped(sessionId: string, error: unknown) {
  if (!(error instanceof SummarySkippedError)) return;
  showToast({
    id: `auto-summary-too-short-${sessionId}`,
    title: "Summary wasn't generated",
    description: error.reason,
  });
}

async function runSummary(
  sessionId: string,
  automatic: boolean,
): Promise<void> {
  const existing = await execute<{
    id: string;
    updated_at: string;
    body: string;
    session_title: string;
  }>(
    `SELECT document.id, document.updated_at, document.body, session.title AS session_title
     FROM session_documents AS document
     JOIN sessions AS session ON session.id = document.session_id AND session.deleted_at IS NULL
     WHERE document.session_id = ? AND document.kind = 'summary' AND document.deleted_at IS NULL
     ORDER BY document.sort_order, document.created_at, document.id LIMIT 1`,
    [sessionId],
  );
  if (
    automatic &&
    existing[0] &&
    hasSummaryContent(existing[0].body, existing[0].session_title)
  )
    return;
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
  const segments = transcripts.flatMap((row) => transcriptSegments(row, names));
  const transcript = segments
    .map((segment) => `${segment.speaker}: ${segment.text}`)
    .join("\n");
  const source = `Notes:\n${text}\n\nTranscript:\n${transcript}`;
  if (!text.trim() && !transcript.trim())
    throw new Error("Add notes or transcribe a recording first.");
  const eligibility = getSummaryEligibility({
    transcriptCount: transcripts.length,
    wordCount: segments.reduce(
      (total, segment) => total + segment.wordCount,
      0,
    ),
    characterCount: Array.from(
      segments
        .map((segment) => segment.text)
        .join(" ")
        .replace(/\s+/gu, " ")
        .trim(),
    ).length,
  });
  if (!eligibility.eligible && eligibility.code === "transcript_too_short")
    throw new SummarySkippedError(eligibility.reason);
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
      WHERE id = ? AND session_id = ? AND updated_at = ? AND body = ? AND deleted_at IS NULL
        AND EXISTS (SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)`,
          params: [
            summary,
            metadata,
            now,
            prior.id,
            sessionId,
            prior.updated_at,
            prior.body,
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
    onError: (error) => {
      if (error instanceof SummarySkippedError) return;
      captureOperationalError(error, { operation: "session_summary" });
    },
  });
  const promise = mutation
    .execute(undefined)
    .finally(() => inflight.delete(sessionId));
  inflight.set(sessionId, promise);
  return promise;
}

export function generateSummaryAfterTranscription(sessionId: string): void {
  // Summary failures are visible in the note; they must never fail audio persistence.
  void summarizeSession(sessionId, { automatic: true }).catch((error) =>
    notifySummarySkipped(sessionId, error),
  );
}

export function automaticSummaryOptions(sessionId: string) {
  return queryOptions({
    queryKey: ["session-auto-summary", sessionId],
    queryFn: async () => {
      await summarizeSession(sessionId, { automatic: true });
      return null;
    },
    retry: false,
    retryOnMount: false,
    staleTime: Infinity,
  });
}

export function useAutomaticSummary(sessionId: string, enabled: boolean) {
  return useQuery({ ...automaticSummaryOptions(sessionId), enabled });
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
