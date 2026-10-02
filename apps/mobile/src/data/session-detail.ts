import { hasSummaryContent, resolveSummaryDocument } from "@anlg/utils/session";

import {
  docToPlainText,
  isPlainTextDoc,
  stripMarkdownTitle,
} from "./note-doc.ts";

export type SessionDetail = {
  id: string;
  title: string;
  createdAt: string;
  noteText: string;
  bodyFormat: "prosemirror_json" | "markdown";
  plainEditable: boolean;
  summary: { title: string; text: string } | null;
};

export type SessionDetailRow = {
  id: string;
  title: string;
  created_at: string;
  raw_body: string;
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
  const note = documentText(row.raw_body, row.raw_body_format);
  const summary = resolveSummaryDocument(
    JSON.parse(row.summary_documents_json) as Array<{
      id: string;
      title: string;
      body: string;
      body_format: string;
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

  return {
    id: row.id,
    title: row.title,
    createdAt: row.created_at,
    noteText: note.text,
    bodyFormat: isMarkdown ? "markdown" : "prosemirror_json",
    plainEditable: isMarkdown || isPlainTextDoc(row.raw_body),
    summary:
      !summary || !hasSummaryContent(summary.body, row.title)
        ? null
        : {
            title: summaryTitle,
            text: summaryText,
          },
  };
}
