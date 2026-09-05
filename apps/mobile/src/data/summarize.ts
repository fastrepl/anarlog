import { fetch } from "expo/fetch";

import { execute, executeTransaction } from "@/db";
import { env } from "@/lib/env";
import { id, nowIso } from "@/lib/ids";
import { readPreferences } from "@/settings/preferences";
import { resolveProvider } from "@/settings/providers";

import { docToPlainText, stripMarkdownTitle } from "./note-doc";
import { buildSummaryPrompt, readSummaryText } from "./summary-model";
import { readBoundedTranscriptionResponse } from "./transcription-response";

export async function summarizeSession(sessionId: string): Promise<void> {
  const [notes, transcripts, existing, preferences, provider] =
    await Promise.all([
      execute<{ body: string; body_format: string }>(
        "SELECT body, body_format FROM session_documents WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL ORDER BY CASE WHEN id = session_id THEN 0 ELSE 1 END, sort_order, id LIMIT 1",
        [sessionId],
      ),
      execute<{ words_json: string }>(
        "SELECT words_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL ORDER BY started_at_ms, id",
        [sessionId],
      ),
      execute<{ id: string; updated_at: string }>(
        "SELECT id, updated_at FROM session_documents WHERE session_id = ? AND kind = 'summary' AND deleted_at IS NULL ORDER BY sort_order, created_at, id LIMIT 1",
        [sessionId],
      ),
      readPreferences(),
      resolveProvider("llm"),
    ]);
  const note = notes[0];
  const text = note
    ? (note.body_format === "markdown"
        ? stripMarkdownTitle(note.body)
        : docToPlainText(note.body)
      ).text
    : "";
  const transcript = transcripts
    .map((row) => {
      const words: unknown = JSON.parse(row.words_json);
      if (!Array.isArray(words))
        throw new Error("This transcript could not be read.");
      return words
        .map((word) => (typeof word?.text === "string" ? word.text : ""))
        .join(" ");
    })
    .join("\n");
  const source = `Notes:\n${text}\n\nTranscript:\n${transcript}`;
  if (!text.trim() && !transcript.trim())
    throw new Error("Add notes or transcribe a recording first.");
  if (source.length > 200_000)
    throw new Error(
      "This meeting is too long to summarize on mobile. Open it on desktop.",
    );
  const anthropic = provider.provider === "anthropic";
  const url =
    provider.provider === "anarlog"
      ? `${env.apiUrl}/llm/chat/completions`
      : `${provider.baseUrl}/${anthropic ? "messages" : "chat/completions"}`;
  const system = buildSummaryPrompt(preferences);
  const request = anthropic
    ? {
        model: provider.model,
        max_tokens: 4096,
        system,
        messages: [{ role: "user", content: source }],
      }
    : {
        model: provider.model || "default",
        stream: false,
        messages: [
          { role: "system", content: system },
          { role: "user", content: source },
        ],
      };
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 120_000);
  let summary: string;
  try {
    const response = await fetch(url, {
      method: "POST",
      signal: controller.signal,
      redirect: "error",
      headers: {
        "Content-Type": "application/json",
        ...(provider.apiKey
          ? anthropic
            ? {
                "x-api-key": provider.apiKey,
                "anthropic-version": "2023-06-01",
              }
            : { Authorization: `Bearer ${provider.apiKey}` }
          : {}),
        ...(provider.provider === "anarlog"
          ? { "x-char-task": "enhance" }
          : {}),
      },
      body: JSON.stringify(request),
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
