import type { MeetingScreenShareCapture } from "@anlg/plugin-detect";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { catalogLocalNoteAttachment, sha256Hex } from "~/session/attachments";
import { formatMeetingPlatform } from "~/stt/meeting-chat-records";

export type MeetingScreenRecord = {
  id: string;
  attachmentId: string;
  platform: string | null;
  width: number;
  height: number;
  capturedAt: string;
};

type MeetingScreenDocumentRow = { id: string; body: string };

const EMPTY_MEETING_SCREEN_RECORDS: MeetingScreenRecord[] = [];
const MEETING_SCREEN_RECORDS_SQL = `
  SELECT id, body
  FROM session_documents
  WHERE session_id = ?
    AND kind = 'meeting_screen'
    AND deleted_at IS NULL
  ORDER BY sort_order ASC, created_at ASC
`;

export function useMeetingScreenRecords(
  sessionId: string,
): MeetingScreenRecord[] {
  const { data = EMPTY_MEETING_SCREEN_RECORDS } = useLiveQuery<
    MeetingScreenDocumentRow,
    MeetingScreenRecord[]
  >({
    sql: MEETING_SCREEN_RECORDS_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.flatMap(parseMeetingScreenRow),
  });

  return sessionId ? data : EMPTY_MEETING_SCREEN_RECORDS;
}

export async function persistMeetingScreenCapture({
  sessionId,
  capture,
}: {
  sessionId: string;
  capture: MeetingScreenShareCapture;
}): Promise<void> {
  if (!capture.jpeg) {
    return;
  }

  const capturedAt = new Date().toISOString();
  const bytes = new Uint8Array(capture.jpeg);
  const filename = `shared-screen-${capturedAt.replace(/[:.]/g, "-")}.jpg`;
  const saved = await fsSyncCommands.attachmentSave(
    sessionId,
    capture.jpeg,
    filename,
  );
  if (saved.status === "error") {
    throw new Error(saved.error);
  }

  const { attachmentId } = saved.data;
  try {
    await catalogLocalNoteAttachment({
      sessionId,
      attachmentId,
      filename,
      contentType: "image/jpeg",
      sizeBytes: bytes.byteLength,
      sha256: await sha256Hex(bytes.buffer),
    });
  } catch (error) {
    await fsSyncCommands
      .attachmentRemove(sessionId, attachmentId)
      .catch(() => undefined);
    throw error;
  }

  const record: MeetingScreenRecord = {
    id: `${sessionId}:meeting-screen:${attachmentId}`,
    attachmentId,
    platform: capture.platform,
    width: capture.width,
    height: capture.height,
    capturedAt,
  };
  const title = capture.platform
    ? `${formatMeetingPlatform(capture.platform)} shared screen`
    : "Shared screen";

  await enqueueDatabaseWrite(`session:${sessionId}`, () =>
    executeTransaction([
      {
        sql: `
          INSERT INTO session_documents (
            id, session_id, kind, title, body_format, body, source_hash,
            generation_metadata_json, sort_order, created_by, updated_by,
            created_at, updated_at, deleted_at
          )
          SELECT
            ?, id, 'meeting_screen', ?, 'json', ?, ?, ?, ?, owner_user_id,
            owner_user_id, ?, ?, NULL
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
          ON CONFLICT(id) DO NOTHING
        `,
        params: [
          record.id,
          title,
          JSON.stringify(record),
          attachmentId,
          JSON.stringify({ source: "meeting_screen_share", version: 1 }),
          Date.now(),
          capturedAt,
          capturedAt,
          sessionId,
        ],
      },
    ]),
  );
}

function parseMeetingScreenRow(
  row: MeetingScreenDocumentRow,
): MeetingScreenRecord[] {
  try {
    const value = JSON.parse(row.body) as Partial<MeetingScreenRecord>;
    if (
      typeof value.attachmentId !== "string" ||
      typeof value.capturedAt !== "string"
    ) {
      return [];
    }
    return [
      {
        id: row.id,
        attachmentId: value.attachmentId,
        platform: typeof value.platform === "string" ? value.platform : null,
        width: Number(value.width) || 0,
        height: Number(value.height) || 0,
        capturedAt: value.capturedAt,
      },
    ];
  } catch {
    return [];
  }
}
