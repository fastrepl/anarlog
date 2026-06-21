import { extractPlainText, flattenTranscript, mergeContent } from "./utils";

export function createSessionSearchableContent(
  row: Record<string, unknown>,
): string {
  return mergeContent([
    flattenSessionSourceApps(row.source_app_json),
    extractPlainText(row.raw_md),
    extractPlainText(row.enhanced_notes_content),
    flattenTranscript(row.transcript),
  ]);
}

function flattenSessionSourceApps(value: unknown): string {
  if (typeof value !== "string" || !value.trim()) {
    return "";
  }

  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) {
      return "";
    }

    return mergeContent(
      parsed.map((item) => {
        if (!item || typeof item !== "object") {
          return "";
        }

        const record = item as Record<string, unknown>;
        return mergeContent([record.name, record.id]);
      }),
    );
  } catch {
    return "";
  }
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
