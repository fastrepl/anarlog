import { extractPlainText, flattenTranscript, mergeContent } from "./utils";

export function createSessionSearchableContent(
  row: Record<string, unknown>,
): string {
  return mergeContent([
    extractPlainText(row.raw_md),
    extractPlainText(row.enhanced_notes_content),
    flattenTranscript(row.transcript),
  ]);
}

export function createHumanSearchableContent(
  row: Record<string, unknown>,
): string {
  return mergeContent([
    row.email,
    row.job_title,
    row.linkedin_username,
    row.memo,
  ]);
}
