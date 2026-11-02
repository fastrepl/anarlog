import { execute, executeTransaction } from "@/db";
import { nowIso } from "@/lib/ids";

export const MAX_MEETING_GLOSSARY_TERMS = 100;
export const MAX_MEETING_GLOSSARY_CHARACTERS = 4_000;

function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

export function normalizeMeetingGlossary(terms: readonly string[]): string[] {
  const normalized: string[] = [];
  const seen = new Set<string>();
  let characters = 0;
  for (const rawTerm of terms) {
    const term = rawTerm.trim().replace(/\s+/gu, " ");
    if (!term) continue;
    if (Array.from(term).length > 100)
      throw new Error("Each glossary term must be 100 characters or fewer.");
    const key = term.toLocaleLowerCase();
    if (seen.has(key)) continue;
    if (normalized.length >= MAX_MEETING_GLOSSARY_TERMS)
      throw new Error("A meeting glossary can contain up to 100 terms.");
    const byteLength = utf8Length(term);
    if (characters + byteLength > MAX_MEETING_GLOSSARY_CHARACTERS)
      throw new Error("A meeting glossary can contain up to 4,000 characters.");
    seen.add(key);
    normalized.push(term);
    characters += byteLength;
  }
  return normalized;
}

export function parseMeetingGlossaryText(text: string): string[] {
  return normalizeMeetingGlossary(text.split(/\r?\n/u));
}

export function mergeTranscriptionKeywords(
  ...groups: readonly (readonly string[])[]
): string[] {
  const merged: string[] = [];
  const seen = new Set<string>();
  let characters = 0;
  for (const group of groups) {
    for (const rawTerm of group) {
      const term = rawTerm.trim().replace(/\s+/gu, " ");
      if (
        !term ||
        Array.from(term).length > 100 ||
        seen.has(term.toLocaleLowerCase())
      )
        continue;
      const byteLength = utf8Length(term);
      if (
        merged.length >= MAX_MEETING_GLOSSARY_TERMS ||
        characters + byteLength > MAX_MEETING_GLOSSARY_CHARACTERS
      )
        return merged;
      seen.add(term.toLocaleLowerCase());
      merged.push(term);
      characters += byteLength;
    }
  }
  return merged;
}

function parseStoredTerms(value: unknown): string[] {
  if (typeof value !== "string") return [];
  try {
    const terms: unknown = JSON.parse(value);
    if (
      !Array.isArray(terms) ||
      !terms.every((term) => typeof term === "string")
    )
      return [];
    return normalizeMeetingGlossary(terms);
  } catch {
    return [];
  }
}

export async function readMeetingGlossary(sessionId: string): Promise<string[]> {
  const rows = await execute<{ terms_json: string | null }>(
    `SELECT CASE WHEN json_valid(metadata_json)
      THEN json_extract(metadata_json, '$.transcription_glossary')
      ELSE NULL END AS terms_json
    FROM sessions WHERE id = ? AND deleted_at IS NULL`,
    [sessionId],
  );
  return parseStoredTerms(rows[0]?.terms_json);
}

export async function saveMeetingGlossary(
  sessionId: string,
  terms: readonly string[],
): Promise<void> {
  const normalized = normalizeMeetingGlossary(terms);
  await executeTransaction([
    {
      sql: `UPDATE sessions
        SET metadata_json = json_set(
          CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
          '$.transcription_glossary', json(?)
        ), updated_at = ?
        WHERE id = ? AND deleted_at IS NULL`,
      params: [JSON.stringify(normalized), nowIso(), sessionId],
    },
  ]);
}
