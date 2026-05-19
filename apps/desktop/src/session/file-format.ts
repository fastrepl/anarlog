export const SESSION_MARKDOWN_SCHEMA_VERSION = 1;
export const SESSION_MARKDOWN_FILE = "session.md";

export type SessionMarkdownParticipant = {
  person_id?: string;
  legacy_human_id?: string;
  name?: string;
  email?: string;
  source?: string;
};

export type SessionMarkdownDocument = {
  schemaVersion: typeof SESSION_MARKDOWN_SCHEMA_VERSION;
  id: string;
  createdAt: string;
  updatedAt?: string;
  title: string;
  folderId?: string;
  eventId?: string;
  event?: Record<string, unknown>;
  participants: SessionMarkdownParticipant[];
  tags: string[];
  notes: string;
  summary: string;
  transcript: string;
};

type SectionKey = "notes" | "summary" | "transcript";

const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/;
const SECTION_TITLES: Record<SectionKey, string> = {
  notes: "Notes",
  summary: "Summary",
  transcript: "Transcript",
};

const SECTION_KEYS_BY_TITLE = new Map<string, SectionKey>(
  Object.entries(SECTION_TITLES).map(([key, title]) => [
    title,
    key as SectionKey,
  ]),
);

function parseFrontmatter(markdown: string): {
  frontmatter: Record<string, unknown>;
  body: string;
} {
  const match = markdown.match(FRONTMATTER_RE);
  if (!match) {
    return { frontmatter: {}, body: markdown };
  }

  const frontmatter: Record<string, unknown> = {};
  for (const line of match[1].split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const separator = trimmed.indexOf(":");
    if (separator === -1) continue;

    const key = trimmed.slice(0, separator).trim();
    const rawValue = trimmed.slice(separator + 1).trim();
    if (!key) continue;
    frontmatter[key] = parseFrontmatterValue(rawValue);
  }

  return {
    frontmatter,
    body: markdown.slice(match[0].length),
  };
}

function parseFrontmatterValue(rawValue: string): unknown {
  if (!rawValue) return "";

  try {
    return JSON.parse(rawValue);
  } catch {
    return rawValue;
  }
}

function stringifyFrontmatterValue(value: unknown): string {
  if (value === undefined) return "";
  return JSON.stringify(value);
}

function addFrontmatterLine(
  lines: string[],
  key: string,
  value: unknown,
): void {
  if (value === undefined) return;
  if (typeof value === "string" && !value) return;
  if (Array.isArray(value) && value.length === 0) return;
  lines.push(`${key}: ${stringifyFrontmatterValue(value)}`);
}

function renderFrontmatter(document: SessionMarkdownDocument): string {
  const lines = ["---"];
  addFrontmatterLine(lines, "schema_version", document.schemaVersion);
  addFrontmatterLine(lines, "id", document.id);
  addFrontmatterLine(lines, "created_at", document.createdAt);
  addFrontmatterLine(lines, "updated_at", document.updatedAt);
  addFrontmatterLine(lines, "title", document.title);
  addFrontmatterLine(lines, "folder_id", document.folderId);
  addFrontmatterLine(lines, "event_id", document.eventId);
  addFrontmatterLine(lines, "event_json", document.event);
  addFrontmatterLine(lines, "participants_json", document.participants);
  addFrontmatterLine(lines, "tags_json", document.tags);
  lines.push("---");
  return lines.join("\n");
}

function sectionHeading(key: SectionKey): string {
  return `# ${SECTION_TITLES[key]}`;
}

function normalizeSectionContent(content: string): string {
  return content.replace(/^\r?\n/, "").trimEnd();
}

function parseSections(
  body: string,
): Pick<SessionMarkdownDocument, "notes" | "summary" | "transcript"> {
  const sections: Record<SectionKey, string> = {
    notes: "",
    summary: "",
    transcript: "",
  };

  const matches = Array.from(
    body.matchAll(/^# (Notes|Summary|Transcript)\s*$/gm),
  );
  if (matches.length === 0) {
    return {
      notes: body.trim(),
      summary: "",
      transcript: "",
    };
  }

  const preamble = body.slice(0, matches[0].index).trim();
  if (preamble) {
    sections.notes = preamble;
  }

  for (let index = 0; index < matches.length; index += 1) {
    const match = matches[index];
    const title = match[1];
    const key = SECTION_KEYS_BY_TITLE.get(title);
    if (!key) continue;

    const start = (match.index ?? 0) + match[0].length;
    const end = matches[index + 1]?.index ?? body.length;
    const content = normalizeSectionContent(body.slice(start, end));
    sections[key] = sections[key] ? `${sections[key]}\n\n${content}` : content;
  }

  return sections;
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function asParticipants(value: unknown): SessionMarkdownParticipant[] {
  if (!Array.isArray(value)) return [];

  return value.flatMap((item) => {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      return [];
    }

    const record = item as Record<string, unknown>;
    return [
      {
        person_id: asString(record.person_id),
        legacy_human_id: asString(record.legacy_human_id),
        name: asString(record.name),
        email: asString(record.email),
        source: asString(record.source),
      },
    ];
  });
}

function asObject(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

export function parseSessionMarkdown(
  markdown: string,
): SessionMarkdownDocument {
  const { frontmatter, body } = parseFrontmatter(markdown);
  const id = asString(frontmatter.id);
  const createdAt = asString(frontmatter.created_at);

  if (!id) {
    throw new Error("session_markdown_missing_id");
  }
  if (!createdAt) {
    throw new Error("session_markdown_missing_created_at");
  }

  return {
    schemaVersion: SESSION_MARKDOWN_SCHEMA_VERSION,
    id,
    createdAt,
    updatedAt: asString(frontmatter.updated_at),
    title: asString(frontmatter.title) ?? "",
    folderId: asString(frontmatter.folder_id),
    eventId: asString(frontmatter.event_id),
    event: asObject(frontmatter.event_json),
    participants: asParticipants(frontmatter.participants_json),
    tags: asStringArray(frontmatter.tags_json),
    ...parseSections(body),
  };
}

export function renderSessionMarkdown(
  document: SessionMarkdownDocument,
): string {
  const parts = [
    renderFrontmatter(document),
    sectionHeading("notes"),
    document.notes.trimEnd(),
    sectionHeading("summary"),
    document.summary.trimEnd(),
    sectionHeading("transcript"),
    document.transcript.trimEnd(),
  ];

  return `${parts.join("\n\n").trimEnd()}\n`;
}
