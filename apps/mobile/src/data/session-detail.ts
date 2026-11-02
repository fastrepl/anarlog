import { hasSummaryContent, resolveSummaryDocument } from "@anlg/utils/session";

import {
  docToPlainText,
  isPlainTextDoc,
  stripMarkdownTitle,
} from "./note-doc.ts";

export type NoteDocument = { id: string; body: string; bodyFormat: string };

export type SessionDetail = {
  document: NoteDocument | null;
  id: string;
  title: string;
  createdAt: string;
  noteText: string;
  bodyFormat: "prosemirror_json" | "markdown";
  plainEditable: boolean;
  summary: { title: string; text: string; outdated: boolean } | null;
};

export type SessionDetailRow = {
  id: string;
  title: string;
  created_at: string;
  note_id: string | null;
  raw_body: string | null;
  raw_body_format: string;
  summary_documents_json: string;
};

function documentText(
  body: string,
  bodyFormat: string,
): {
  title: string;
  text: string;
} {
  return bodyFormat === "markdown"
    ? stripMarkdownTitle(body)
    : docToPlainText(body);
}

export function mapSessionDetailRows(
  rows: SessionDetailRow[],
): SessionDetail | null {
  const row = rows[0];
  if (!row) return null;
  const isMarkdown = row.raw_body_format === "markdown";
  const note = documentText(row.raw_body ?? "", row.raw_body_format);
  const summary = resolveSummaryDocument(
    JSON.parse(row.summary_documents_json) as Array<{
      id: string;
      title: string;
      body: string;
      body_format: string;
      generation_metadata_json?: string;
      kind: string;
      template_id: string;
      sort_order: number;
    }>,
    row.title,
  );
  const summaryDocument = documentText(
    summary?.body ?? "",
    summary?.body_format ?? "prosemirror_json",
  );
  const summaryTitle =
    summary?.title.trim() || summaryDocument.title.trim() || "Summary";
  const summaryText =
    summaryDocument.text.trim() ||
    (summaryDocument.title.trim() !== summaryTitle
      ? summaryDocument.title.trim()
      : "");
  let summaryOutdated = false;
  if (summary?.generation_metadata_json) {
    try {
      const metadata: unknown = JSON.parse(summary.generation_metadata_json);
      summaryOutdated = Boolean(
        metadata &&
        typeof metadata === "object" &&
        "transcript_stale" in metadata &&
        metadata.transcript_stale === true,
      );
    } catch {
      summaryOutdated = false;
    }
  }

  return {
    document:
      row.note_id === null || row.raw_body === null
        ? null
        : {
            id: row.note_id,
            body: row.raw_body,
            bodyFormat: row.raw_body_format,
          },
    id: row.id,
    title: row.title,
    createdAt: row.created_at,
    noteText: note.text,
    bodyFormat: isMarkdown ? "markdown" : "prosemirror_json",
    plainEditable:
      row.note_id !== null &&
      row.raw_body !== null &&
      (isMarkdown || isPlainTextDoc(row.raw_body)),
    summary:
      !summary || !hasSummaryContent(summary.body, row.title)
        ? null
        : {
            title: summaryTitle,
            text: summaryText,
            outdated: summaryOutdated,
          },
  };
}
