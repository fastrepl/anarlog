import { queryOptions, useMutationState } from "@tanstack/react-query";
import { fetch } from "expo/fetch";

import {
  defaultSummaryDocumentId,
  hasSummaryContent,
  resolveSummaryDocument,
} from "@anlg/utils/session";
import { getSummaryEligibility } from "@anlg/utils/summary-eligibility";

import { execute, executeTransaction } from "@/db";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { nowIso } from "@/lib/ids";
import { queryClient } from "@/lib/query-client";
import { showToast } from "@/lib/toast";
import { readPreferences } from "@/settings/preferences";
import { resolveProvider } from "@/settings/providers";

import { docToPlainText, stripMarkdownTitle } from "./note-doc";
import { summaryRequest } from "./provider-summary";
import { PENDING_SUMMARY_PREFIX } from "./summary-job";
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
    deleted_at: string | null;
    body: string;
    session_title: string;
    kind: string;
    template_id: string;
    sort_order: number;
  }>(
    `SELECT document.id, document.updated_at, document.deleted_at, document.body, document.kind, document.template_id, document.sort_order, session.title AS session_title
     FROM session_documents AS document
     JOIN sessions AS session ON session.id = document.session_id AND session.deleted_at IS NULL
     WHERE document.session_id = ? AND document.kind IN ('summary', 'template_output')
     ORDER BY document.sort_order, document.id`,
    [sessionId],
  );
  const prior = resolveSummaryDocument(
    existing.filter((document) => document.deleted_at === null),
    existing[0]?.session_title,
  );
  const target =
    prior ??
    existing.find(
      (document) => document.id === defaultSummaryDocumentId(sessionId),
    );
  if (automatic && prior && hasSummaryContent(prior.body, prior.session_title))
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
  });
  const [changed] = await executeTransaction([
    target
      ? {
          sql: `UPDATE session_documents SET body = ?, body_format = 'markdown', generation_metadata_json = ?, updated_at = ?, deleted_at = NULL
      WHERE id = ? AND session_id = ? AND updated_at = ? AND body = ? AND deleted_at IS ?
        AND EXISTS (SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)`,
          params: [
            summary,
            metadata,
            now,
            target.id,
            sessionId,
            target.updated_at,
            target.body,
            target.deleted_at,
            sessionId,
          ],
        }
      : {
          sql: `INSERT INTO session_documents (id, workspace_id, session_id, kind, title, body_format, body, generation_metadata_json, created_at, updated_at)
      SELECT ?, workspace_id, id, 'summary', 'Summary', 'markdown', ?, ?, ?, ? FROM sessions WHERE id = ? AND deleted_at IS NULL
        AND NOT EXISTS (SELECT 1 FROM session_documents WHERE session_id = ? AND kind IN ('summary', 'template_output') AND deleted_at IS NULL)
        ON CONFLICT(id) DO NOTHING`,
          params: [
            defaultSummaryDocumentId(sessionId),
            summary,
            metadata,
            now,
            now,
            sessionId,
            sessionId,
          ],
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
      const [job] = await execute<{ value_json: string }>(
        "SELECT value_json FROM app_settings WHERE id = ?",
        [`${PENDING_SUMMARY_PREFIX}${sessionId}`],
      );
      const finish = async () => {
        if (!job) return;
        await executeTransaction([
          {
            sql: "DELETE FROM app_settings WHERE id = ? AND value_json = ?",
            params: [`${PENDING_SUMMARY_PREFIX}${sessionId}`, job.value_json],
          },
        ]);
      };
      try {
        await runSummary(sessionId, automatic);
        await finish();
      } catch (error) {
        if (error instanceof SummarySkippedError) await finish();
        throw error;
      }
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

export function summaryRecoveryOptions(sessionId: string, enabled = true) {
  return queryOptions({
    queryKey: ["session-summary-recovery", sessionId],
    enabled,
    queryFn: async () => {
      // Only a local transcription transaction can authorize crash recovery.
      const jobs = await execute<{ value_json: string }>(
        `SELECT setting.value_json FROM app_settings AS setting
         WHERE setting.id = ? AND EXISTS (
           SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL
         ) AND EXISTS (
           SELECT 1 FROM transcripts WHERE session_id = ? AND deleted_at IS NULL
         ) AND EXISTS (
           SELECT 1 FROM session_attachments WHERE session_id = ?
             AND source_type = 'session_audio' AND deleted_at IS NULL
             AND json_extract(metadata_json, '$.transcript_status') = 'complete'
         )`,
        [
          `${PENDING_SUMMARY_PREFIX}${sessionId}`,
          sessionId,
          sessionId,
          sessionId,
        ],
      );
      if (jobs.length) await summarizeSession(sessionId, { automatic: true });
      return null;
    },
    retry: false,
    staleTime: Infinity,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
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
