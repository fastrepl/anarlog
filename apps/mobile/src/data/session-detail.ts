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
  summary_id: string;
  summary_title: string;
  summary_body: string;
  summary_body_format: string;
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
  const summaryDocument = documentText(
    row.summary_body,
    row.summary_body_format,
  );
  const summaryTitle =
    row.summary_title.trim() || summaryDocument.title.trim() || "Summary";
  const summaryText =
    summaryDocument.text.trim() ||
    (summaryDocument.title.trim() !== summaryTitle
      ? summaryDocument.title.trim()
      : "");
  const hasSummaryContent = summaryTitle !== "Summary" || summaryText !== "";

  return {
    id: row.id,
    title: row.title,
    createdAt: row.created_at,
    noteText: note.text,
    bodyFormat: isMarkdown ? "markdown" : "prosemirror_json",
    plainEditable: isMarkdown || isPlainTextDoc(row.raw_body),
    summary:
      row.summary_id === "" || !hasSummaryContent
        ? null
        : {
            title: summaryTitle,
            text: summaryText,
          },
  };
}
