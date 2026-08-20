import type { ImportTextFile } from "@anlg/plugin-importer";

export type ImportedMeeting = {
  externalId: string;
  title: string;
  startedAt: string;
  endedAt: string;
  sourceUrl: string;
  noteMarkdown: string;
  transcript: Array<{
    speaker: string;
    text: string;
    startMs: number;
    endMs: number;
  }>;
  attendees: Array<{ name: string; email: string }>;
  actionItems: string[];
};

type JsonRecord = Record<string, unknown>;

const MEETING_ARRAY_KEYS = [
  "meetings",
  "conversations",
  "documents",
  "notes",
  "recordings",
  "results",
  "items",
  "data",
];

export function parseMeetingExport(file: ImportTextFile): ImportedMeeting[] {
  const extension = file.name.split(".").pop()?.toLowerCase();
  const fallbackTitle = file.name.replace(/\.[^.]+$/u, "");

  if (extension === "json") {
    return parseJsonExport(file.content, fallbackTitle);
  }
  if (extension === "csv") {
    return parseCsvExport(file.content, fallbackTitle);
  }
  if (extension === "srt" || extension === "vtt") {
    return [
      emptyMeeting(fallbackTitle, {
        transcript: parseCaptionTranscript(file.content),
      }),
    ];
  }
  if (extension === "txt") {
    return [
      emptyMeeting(fallbackTitle, {
        transcript: parseTextTranscript(file.content),
      }),
    ];
  }

  return [emptyMeeting(fallbackTitle, { noteMarkdown: file.content.trim() })];
}

function parseJsonExport(content: string, fallbackTitle: string) {
  const parsed = JSON.parse(content) as unknown;
  const candidates = findMeetingCandidates(parsed);
  if (candidates.length === 0) {
    throw new Error("This JSON export does not contain any meetings");
  }

  return candidates.map((candidate, index) =>
    normalizeMeeting(candidate, `${fallbackTitle} ${index + 1}`),
  );
}

function findMeetingCandidates(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  const record = asRecord(value);
  if (!record) return [];

  for (const key of MEETING_ARRAY_KEYS) {
    const candidate = record[key];
    if (Array.isArray(candidate)) return candidate;
  }

  return [value];
}

function normalizeMeeting(value: unknown, fallbackTitle: string) {
  const record = asRecord(value);
  if (!record) {
    return emptyMeeting(fallbackTitle, {
      noteMarkdown: typeof value === "string" ? value : JSON.stringify(value),
    });
  }

  if (asRecord(record.mapping)) {
    return normalizeChatGptConversation(record, fallbackTitle);
  }

  const summary = firstText(record, [
    "summary",
    "ai_summary",
    "aiSummary",
    "overview",
    "synopsis",
  ]);
  const notes = firstText(record, [
    "notes",
    "note",
    "content",
    "markdown",
    "private_notes",
    "privateNotes",
    "enhanced_notes",
    "enhancedNotes",
    "meeting_notes",
    "meetingNotes",
  ]);
  const transcriptValue = firstValue(record, [
    "transcript",
    "transcription",
    "transcriptSegments",
    "utterances",
    "segments",
    "sentences",
    "dialogue",
  ]);
  const transcript = parseTranscriptValue(transcriptValue);
  const noteMarkdown = composeNote(summary, notes, record, transcript.length);

  return emptyMeeting(
    firstText(record, [
      "title",
      "name",
      "subject",
      "meeting_title",
      "meetingTitle",
      "call_title",
      "callTitle",
      "recording_title",
      "recordingTitle",
      "meeting_name",
      "meetingName",
      "topic",
    ]) || fallbackTitle,
    {
      externalId: firstText(record, [
        "id",
        "meeting_id",
        "meetingId",
        "transcript_id",
        "transcriptId",
        "document_id",
        "documentId",
        "call_id",
        "callId",
        "recording_id",
        "recordingId",
        "conversation_id",
        "conversationId",
        "uuid",
      ]),
      startedAt: toIsoDate(
        firstValue(record, [
          "started_at",
          "startedAt",
          "start_time",
          "startTime",
          "meeting_date",
          "meetingDate",
          "recording_date",
          "recordingDate",
          "date",
          "created_at",
          "createdAt",
        ]),
      ),
      endedAt: toIsoDate(
        firstValue(record, ["ended_at", "endedAt", "end_time", "endTime"]),
      ),
      sourceUrl: firstText(record, [
        "url",
        "meeting_url",
        "meetingUrl",
        "call_url",
        "callUrl",
        "recording_url",
        "recordingUrl",
        "share_url",
        "shareUrl",
        "granola_url",
        "granolaUrl",
      ]),
      noteMarkdown,
      transcript,
      attendees: parseAttendees(
        firstValue(record, ["attendees", "participants", "people", "invitees"]),
      ),
      actionItems: parseActionItems(
        firstValue(record, [
          "action_items",
          "actionItems",
          "tasks",
          "takeaways",
          "next_steps",
          "nextSteps",
        ]),
      ),
    },
  );
}

function normalizeChatGptConversation(
  record: JsonRecord,
  fallbackTitle: string,
): ImportedMeeting {
  const mapping = asRecord(record.mapping) ?? {};
  const messages = Object.values(mapping)
    .map((node) => asRecord(node))
    .map((node) => asRecord(node?.message))
    .filter((message): message is JsonRecord => Boolean(message))
    .sort(
      (left, right) =>
        numericValue(left.create_time) - numericValue(right.create_time),
    );
  const text = messages
    .flatMap((message) => {
      const content = asRecord(message.content);
      return Array.isArray(content?.parts)
        ? content.parts.filter(
            (part): part is string => typeof part === "string",
          )
        : [];
    })
    .join("\n\n")
    .trim();

  return emptyMeeting(firstText(record, ["title"]) || fallbackTitle, {
    externalId: firstText(record, ["id", "conversation_id"]),
    startedAt: toIsoDate(record.create_time),
    noteMarkdown: text,
  });
}

function parseCsvExport(content: string, fallbackTitle: string) {
  const rows = parseCsvRows(content);
  if (rows.length < 2) {
    throw new Error("This CSV export does not contain any meeting rows");
  }

  const headers = rows[0]!.map((header) => header.trim());
  return rows.slice(1).flatMap((row, index) => {
    if (row.every((cell) => !cell.trim())) return [];
    const record = Object.fromEntries(
      headers.map((header, column) => [header, row[column] ?? ""]),
    );
    return [normalizeMeeting(record, `${fallbackTitle} ${index + 1}`)];
  });
}

export function parseCsvRows(input: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;

  for (let index = 0; index < input.length; index += 1) {
    const character = input[index]!;
    if (character === '"') {
      if (quoted && input[index + 1] === '"') {
        cell += '"';
        index += 1;
      } else {
        quoted = !quoted;
      }
    } else if (character === "," && !quoted) {
      row.push(cell);
      cell = "";
    } else if ((character === "\n" || character === "\r") && !quoted) {
      if (character === "\r" && input[index + 1] === "\n") index += 1;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else {
      cell += character;
    }
  }

  if (cell || row.length > 0) {
    row.push(cell);
    rows.push(row);
  }
  return rows;
}

function parseCaptionTranscript(content: string) {
  const blocks = content
    .replace(/^WEBVTT[^\n]*\n/u, "")
    .split(/\r?\n\s*\r?\n/u);
  return blocks.flatMap((block) => {
    const lines = block
      .split(/\r?\n/u)
      .map((line) => line.trim())
      .filter(Boolean);
    const timingIndex = lines.findIndex((line) => line.includes("-->"));
    if (timingIndex === -1) return [];
    const [start, end] = lines[timingIndex]!.split("-->");
    const rawText = lines.slice(timingIndex + 1).join(" ");
    const voiceMatch = rawText.match(/^<v\s+([^>]+)>(.*)$/u);
    const speakerMatch = rawText.match(/^([^:]{1,60}):\s+(.+)$/u);
    return [
      {
        speaker: voiceMatch?.[1]?.trim() ?? speakerMatch?.[1]?.trim() ?? "",
        text: stripCaptionMarkup(
          voiceMatch?.[2] ?? speakerMatch?.[2] ?? rawText,
        ),
        startMs: parseTimestamp(start?.trim() ?? ""),
        endMs: parseTimestamp(end?.trim().split(/\s/u)[0] ?? ""),
      },
    ];
  });
}

function parseTextTranscript(content: string) {
  return content
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line, index) => {
      const match = line.match(/^([^:]{1,60}):\s+(.+)$/u);
      return {
        speaker: match?.[1]?.trim() ?? "",
        text: match?.[2]?.trim() ?? line,
        startMs: index * 1_000,
        endMs: (index + 1) * 1_000,
      };
    });
}

function parseTranscriptValue(value: unknown): ImportedMeeting["transcript"] {
  if (typeof value === "string") return parseTextTranscript(value);
  const record = asRecord(value);
  if (record) {
    return parseTranscriptValue(
      firstValue(record, [
        "transcriptSegments",
        "segments",
        "utterances",
        "sentences",
        "items",
        "text",
      ]),
    );
  }
  if (!Array.isArray(value)) return [];

  return value.flatMap((segment, index) => {
    if (typeof segment === "string") {
      return [
        {
          speaker: "",
          text: segment,
          startMs: index * 1_000,
          endMs: (index + 1) * 1_000,
        },
      ];
    }
    const item = asRecord(segment);
    if (!item) return [];
    const text = firstText(item, ["text", "content", "transcript", "sentence"]);
    if (!text) return [];
    const startMs = toMilliseconds(
      firstValue(item, [
        "start_ms",
        "startMs",
        "start_time",
        "start",
        "offset",
      ]),
    );
    const endMs = toMilliseconds(
      firstValue(item, ["end_ms", "endMs", "end_time", "end"]),
    );
    return [
      {
        speaker: firstText(item, [
          "speaker_name",
          "speakerName",
          "speaker",
          "participant",
          "name",
        ]),
        text,
        startMs: startMs ?? index * 1_000,
        endMs: endMs ?? (startMs ?? index * 1_000) + 1_000,
      },
    ];
  });
}

function parseAttendees(value: unknown) {
  if (!Array.isArray(value)) return [];
  return value.flatMap((attendee) => {
    if (typeof attendee === "string") {
      return [
        attendee.includes("@")
          ? { name: "", email: attendee }
          : { name: attendee, email: "" },
      ];
    }
    const record = asRecord(attendee);
    if (!record) return [];
    const name = firstText(record, ["name", "display_name", "displayName"]);
    const email = firstText(record, ["email", "email_address", "emailAddress"]);
    return name || email ? [{ name, email }] : [];
  });
}

function parseActionItems(value: unknown): string[] {
  if (typeof value === "string") {
    return value
      .split(/\r?\n/u)
      .map((item) => item.replace(/^[-*]\s*/u, "").trim())
      .filter(Boolean);
  }
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (typeof item === "string") return item.trim() ? [item.trim()] : [];
    const record = asRecord(item);
    const text = record
      ? firstText(record, ["text", "title", "description", "task", "content"])
      : "";
    return text ? [text] : [];
  });
}

function composeNote(
  summary: string,
  notes: string,
  record: JsonRecord,
  transcriptCount: number,
) {
  const sections: string[] = [];
  if (summary) sections.push(`## Summary\n\n${summary}`);
  if (notes && notes !== summary) sections.push(`## Notes\n\n${notes}`);
  if (sections.length === 0 && transcriptCount === 0) {
    sections.push(`\`\`\`json\n${JSON.stringify(record, null, 2)}\n\`\`\``);
  }
  return sections.join("\n\n");
}

function emptyMeeting(
  title: string,
  overrides: Partial<ImportedMeeting> = {},
): ImportedMeeting {
  return {
    externalId: "",
    title,
    startedAt: "",
    endedAt: "",
    sourceUrl: "",
    noteMarkdown: "",
    transcript: [],
    attendees: [],
    actionItems: [],
    ...overrides,
  };
}

function firstValue(record: JsonRecord, keys: string[]) {
  for (const key of keys) {
    if (record[key] != null) return record[key];
  }

  const normalizedEntries = Object.entries(record).map(
    ([key, value]) => [normalizeFieldName(key), value] as const,
  );
  for (const key of keys) {
    const match = normalizedEntries.find(
      ([candidate]) => candidate === normalizeFieldName(key),
    );
    if (match?.[1] != null) return match[1];
  }
  return undefined;
}

function normalizeFieldName(value: string) {
  return value.toLowerCase().replace(/[^a-z0-9]/gu, "");
}

function firstText(record: JsonRecord, keys: string[]): string {
  const value = firstValue(record, keys);
  if (typeof value === "string") return value.trim();
  if (typeof value === "number") return String(value);
  if (Array.isArray(value)) {
    return value
      .map((item) => {
        if (typeof item === "string") return item.trim();
        const nested = asRecord(item);
        return nested
          ? firstText(nested, ["text", "content", "value", "name"])
          : "";
      })
      .filter(Boolean)
      .join("\n");
  }
  const nested = asRecord(value);
  return nested ? firstText(nested, ["text", "content", "value", "name"]) : "";
}

function asRecord(value: unknown): JsonRecord | null {
  return value != null && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonRecord)
    : null;
}

function toIsoDate(value: unknown) {
  if (typeof value !== "string" && typeof value !== "number") return "";
  const milliseconds =
    typeof value === "number" && value < 10_000_000_000 ? value * 1_000 : value;
  const date = new Date(milliseconds);
  return Number.isNaN(date.getTime()) ? "" : date.toISOString();
}

function toMilliseconds(value: unknown) {
  if (typeof value === "number") return value < 100_000 ? value * 1_000 : value;
  if (typeof value !== "string" || !value.trim()) return undefined;
  if (value.includes(":")) return parseTimestamp(value);
  const parsed = Number(value);
  return Number.isFinite(parsed)
    ? parsed < 100_000
      ? parsed * 1_000
      : parsed
    : undefined;
}

function numericValue(value: unknown) {
  return typeof value === "number" ? value : Number(value) || 0;
}

function parseTimestamp(value: string) {
  const parts = value.replace(",", ".").split(":").map(Number);
  if (parts.some((part) => !Number.isFinite(part))) return 0;
  const seconds = parts.pop() ?? 0;
  const minutes = parts.pop() ?? 0;
  const hours = parts.pop() ?? 0;
  return Math.round((hours * 3_600 + minutes * 60 + seconds) * 1_000);
}

function stripCaptionMarkup(value: string) {
  return value.replace(/<[^>]+>/gu, "").trim();
}
