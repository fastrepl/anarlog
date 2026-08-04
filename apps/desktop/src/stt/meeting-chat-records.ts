import type { MeetingCapturedChatMessage } from "@anlg/plugin-detect";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

export type MeetingChatDocumentRow = {
  id: string;
  body: string;
  created_at: string;
};

export type MeetingChatRecord = MeetingCapturedChatMessage & {
  capturedAt: string;
};

const EMPTY_MEETING_CHAT_RECORDS: MeetingChatRecord[] = [];
export const MAX_MEETING_CHAT_RECORDS = 1000;
export const MAX_MEETING_CHAT_RECORD_BYTES = 16 * 1024;
const textEncoder = new TextEncoder();

function jsonStringBytes(value: string): number {
  let bytes = 2;
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code === 0x22 || code === 0x5c) {
      bytes += 2;
    } else if (code < 0x20) {
      bytes += [0x08, 0x09, 0x0a, 0x0c, 0x0d].includes(code) ? 2 : 6;
    } else if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (
      code >= 0xd800 &&
      code <= 0xdbff &&
      index + 1 < value.length &&
      value.charCodeAt(index + 1) >= 0xdc00 &&
      value.charCodeAt(index + 1) <= 0xdfff
    ) {
      bytes += 4;
      index += 1;
    } else if (code >= 0xd800 && code <= 0xdfff) {
      bytes += 6;
    } else {
      bytes += 3;
    }
    if (bytes > MAX_MEETING_CHAT_RECORD_BYTES) {
      return bytes;
    }
  }
  return bytes;
}

function meetingChatRecordFits(record: MeetingChatRecord): boolean {
  let bytes =
    '{"id":,"platform":,"surface":,"sender":,"timestamp":,"direction":,"text":,"links":[],"capturedAt":}'
      .length;
  const addString = (value: string) => {
    bytes += jsonStringBytes(value);
    return bytes <= MAX_MEETING_CHAT_RECORD_BYTES;
  };

  if (
    !addString(record.id) ||
    !addString(record.platform) ||
    !addString(record.surface) ||
    (record.sender === null
      ? ((bytes += 4), false)
      : !addString(record.sender)) ||
    (record.timestamp === null
      ? ((bytes += 4), false)
      : !addString(record.timestamp)) ||
    (record.direction === null
      ? ((bytes += 4), false)
      : !addString(record.direction)) ||
    !addString(record.text) ||
    !addString(record.capturedAt)
  ) {
    return false;
  }

  for (const [index, link] of record.links.entries()) {
    if (index > 0) bytes += 1;
    if (!addString(link)) return false;
  }
  return bytes <= MAX_MEETING_CHAT_RECORD_BYTES;
}

const MEETING_CHAT_RECORDS_SQL = `
  SELECT id, body, created_at
  FROM (
    SELECT id, body, created_at, sort_order
    FROM session_documents
    WHERE session_id = ?
      AND kind = 'meeting_chat'
      AND deleted_at IS NULL
      AND length(CAST(body AS BLOB)) <= ${MAX_MEETING_CHAT_RECORD_BYTES}
    ORDER BY sort_order DESC, created_at DESC, id DESC
    LIMIT ${MAX_MEETING_CHAT_RECORDS}
  )
  ORDER BY sort_order, created_at, id
`;

const MEETING_PLATFORM_LABELS = {
  zoom: "Zoom",
  googleMeet: "Google Meet",
  microsoftTeams: "Microsoft Teams",
  slack: "Slack",
  discord: "Discord",
  webex: "Webex",
  unknown: "Meeting app",
} satisfies Record<MeetingCapturedChatMessage["platform"], string>;

export function useMeetingChatRecords(sessionId: string): MeetingChatRecord[] {
  const { data = EMPTY_MEETING_CHAT_RECORDS } = useLiveQuery<
    MeetingChatDocumentRow,
    MeetingChatRecord[]
  >({
    sql: MEETING_CHAT_RECORDS_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: parseMeetingChatRows,
  });

  return sessionId ? data : EMPTY_MEETING_CHAT_RECORDS;
}

export async function loadMeetingChatRecords(
  sessionId: string,
): Promise<MeetingChatRecord[]> {
  if (!sessionId) {
    return [];
  }

  const rows = await liveQueryClient.execute<MeetingChatDocumentRow>(
    MEETING_CHAT_RECORDS_SQL,
    [sessionId],
  );
  return parseMeetingChatRows(rows);
}

export function persistMeetingChatRecords({
  sessionId,
  entries,
}: {
  sessionId: string;
  entries: Array<{
    message: MeetingCapturedChatMessage;
    sourceSignature: string;
  }>;
}): Promise<string[]> {
  if (entries.length === 0) {
    return Promise.resolve([]);
  }

  return enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const capturedAt = new Date().toISOString();
    const capturedAtMs = Date.now();
    const handledSignatures: string[] = [];
    const rows = entries
      .slice(-MAX_MEETING_CHAT_RECORDS)
      .flatMap(({ message, sourceSignature }, index) => {
        handledSignatures.push(sourceSignature);
        const record: MeetingChatRecord = {
          id: message.id,
          platform: message.platform,
          surface: message.surface,
          sender: message.sender,
          timestamp: message.timestamp,
          direction: message.direction,
          text: message.text,
          links: message.links,
          capturedAt,
        };
        if (!meetingChatRecordFits(record)) {
          console.warn("[listener] skipped oversized meeting chat message");
          return [];
        }
        const sourceHash = createSourceHash(sourceSignature);
        const body = JSON.stringify(record);

        return [
          {
            id: `${sessionId}:meeting-chat:${sourceHash}`,
            sourceHash,
            sourceSignature,
            title: `${formatMeetingPlatform(message.platform)} chat`,
            body,
            sortOrder: capturedAtMs * 100 + index,
          },
        ];
      });

    if (rows.length > 0) {
      await executeTransaction(
        rows.map((row) => ({
          sql: `
            INSERT INTO session_documents (
              id, session_id, kind, title, body_format, body, source_hash,
              generation_metadata_json, sort_order, created_by, updated_by,
              created_at, updated_at, deleted_at
            )
            SELECT
              ?, id, 'meeting_chat', ?, 'json', ?, ?, ?, ?, owner_user_id,
              owner_user_id, ?, ?, NULL
            FROM sessions
            WHERE id = ? AND deleted_at IS NULL
            ON CONFLICT(id) DO NOTHING
          `,
          params: [
            row.id,
            row.title,
            row.body,
            row.sourceHash,
            JSON.stringify({ source: "meeting_ax", version: 1 }),
            row.sortOrder,
            capturedAt,
            capturedAt,
            sessionId,
          ],
        })),
      );
    }

    return handledSignatures;
  });
}

export function parseMeetingChatDocument(
  row: MeetingChatDocumentRow,
): MeetingChatRecord[] {
  if (textEncoder.encode(row.body).byteLength > MAX_MEETING_CHAT_RECORD_BYTES) {
    return [];
  }
  try {
    const value = JSON.parse(row.body) as Partial<MeetingChatRecord>;
    if (
      typeof value.id !== "string" ||
      !isMeetingPlatform(value.platform) ||
      !isMeetingSurface(value.surface) ||
      typeof value.text !== "string" ||
      !Array.isArray(value.links)
    ) {
      return [];
    }

    return [
      {
        id: value.id,
        platform: value.platform,
        surface: value.surface,
        sender: typeof value.sender === "string" ? value.sender : null,
        timestamp: typeof value.timestamp === "string" ? value.timestamp : null,
        direction: isMeetingChatDirection(value.direction)
          ? value.direction
          : null,
        text: value.text,
        links: value.links.filter(
          (link): link is string =>
            typeof link === "string" && /^https?:\/\//.test(link),
        ),
        capturedAt:
          typeof value.capturedAt === "string"
            ? value.capturedAt
            : row.created_at,
      },
    ];
  } catch {
    return [];
  }
}

function parseMeetingChatRows(rows: MeetingChatDocumentRow[]) {
  return rows
    .slice(-MAX_MEETING_CHAT_RECORDS)
    .flatMap(parseMeetingChatDocument);
}

function isMeetingPlatform(
  value: unknown,
): value is MeetingCapturedChatMessage["platform"] {
  return (
    typeof value === "string" &&
    Object.prototype.hasOwnProperty.call(MEETING_PLATFORM_LABELS, value)
  );
}

function isMeetingSurface(
  value: unknown,
): value is MeetingCapturedChatMessage["surface"] {
  return value === "native" || value === "web" || value === "unknown";
}

function isMeetingChatDirection(
  value: unknown,
): value is NonNullable<MeetingCapturedChatMessage["direction"]> {
  return value === "incoming" || value === "outgoing";
}

export function formatMeetingPlatform(
  platform: MeetingCapturedChatMessage["platform"],
) {
  return MEETING_PLATFORM_LABELS[platform];
}

export function formatMeetingChatRecordsAsMarkdown(
  records: MeetingChatRecord[],
) {
  return records
    .map((record) => {
      const direction =
        record.direction === "outgoing"
          ? "sent"
          : record.direction === "incoming"
            ? "received"
            : null;
      const metadata = [
        formatMeetingPlatform(record.platform),
        record.timestamp,
        record.sender,
        direction,
      ]
        .filter((value): value is string => Boolean(value))
        .join(" · ");
      const text = record.text.replace(/\n/g, "\n  ");
      return `- ${metadata}\n  ${text}`;
    })
    .join("\n");
}

export function formatMeetingChatContext(records: MeetingChatRecord[]) {
  const markdown = formatMeetingChatRecordsAsMarkdown(records);
  return markdown ? `## Meeting chat\n${markdown}` : "";
}

function createSourceHash(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let first = 0xcbf29ce484222325n;
  let second = 0x84222325cbf29ce4n;

  for (const byte of bytes) {
    first = BigInt.asUintN(64, (first ^ BigInt(byte)) * 0x100000001b3n);
    second = BigInt.asUintN(64, (second ^ BigInt(byte)) * 0x100000001b3n);
  }

  return [first, second]
    .map((hash) => hash.toString(16).padStart(16, "0"))
    .join("");
}
