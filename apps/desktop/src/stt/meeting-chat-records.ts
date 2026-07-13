import type { MeetingCapturedChatMessage } from "@hypr/plugin-detect";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";

type MeetingChatDocumentRow = {
  id: string;
  body: string;
  created_at: string;
};

export type MeetingChatRecord = MeetingCapturedChatMessage & {
  capturedAt: string;
};

const EMPTY_MEETING_CHAT_RECORDS: MeetingChatRecord[] = [];

export function useMeetingChatRecords(sessionId: string): MeetingChatRecord[] {
  const { data = EMPTY_MEETING_CHAT_RECORDS } = useLiveQuery<
    MeetingChatDocumentRow,
    MeetingChatRecord[]
  >({
    sql: `
      SELECT id, body, created_at
      FROM session_documents
      WHERE session_id = ?
        AND kind = 'meeting_chat'
        AND deleted_at IS NULL
      ORDER BY sort_order, created_at, id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.flatMap(parseMeetingChatDocument),
  });

  return sessionId ? data : EMPTY_MEETING_CHAT_RECORDS;
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
    const rows = entries.map(({ message, sourceSignature }, index) => {
      const sourceHash = createSourceHash(sourceSignature);
      const record: MeetingChatRecord = {
        ...message,
        capturedAt,
      };

      return {
        id: `${sessionId}:meeting-chat:${sourceHash}`,
        sourceHash,
        sourceSignature,
        title: `${formatMeetingPlatform(message.platform)} chat`,
        body: JSON.stringify(record),
        sortOrder: capturedAtMs * 100 + index,
      };
    });

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

    return rows.map((row) => row.sourceSignature);
  });
}

function parseMeetingChatDocument(
  row: MeetingChatDocumentRow,
): MeetingChatRecord[] {
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

function isMeetingPlatform(
  value: unknown,
): value is MeetingCapturedChatMessage["platform"] {
  return value === "zoom" || value === "slack";
}

function isMeetingSurface(
  value: unknown,
): value is MeetingCapturedChatMessage["surface"] {
  return value === "native" || value === "web";
}

function isMeetingChatDirection(
  value: unknown,
): value is NonNullable<MeetingCapturedChatMessage["direction"]> {
  return value === "incoming" || value === "outgoing";
}

function formatMeetingPlatform(
  platform: MeetingCapturedChatMessage["platform"],
) {
  return platform === "zoom" ? "Zoom" : "Slack";
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
