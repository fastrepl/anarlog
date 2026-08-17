import { useLiveQuery } from "@/db";

import {
  mapNoteAttachmentRows,
  type NoteAttachment,
  type NoteAttachmentRow,
} from "./note-attachment-catalog-model";

export type { NoteAttachment } from "./note-attachment-catalog-model";

const NOTE_ATTACHMENTS_SQL = `
SELECT
  attachment.id,
  attachment.source_id,
  attachment.filename,
  attachment.relative_path,
  attachment.content_type,
  attachment.size_bytes,
  attachment.sha256,
  attachment.created_at,
  CASE WHEN local_state.availability = 'present' THEN 1 ELSE 0 END AS available_locally,
  COALESCE(local_state.relative_path, '') AS local_relative_path,
  attachment.cloud_object_key
FROM session_attachments AS attachment
LEFT JOIN attachment_local_state AS local_state
  ON local_state.attachment_id = attachment.id
WHERE attachment.session_id = ?
  AND attachment.source_type = 'note_upload'
  AND attachment.deleted_at IS NULL
ORDER BY attachment.created_at, attachment.id
`;

export function useNoteAttachments(sessionId: string): NoteAttachment[] {
  const { data } = useLiveQuery<NoteAttachmentRow, NoteAttachment[]>({
    sql: NOTE_ATTACHMENTS_SQL,
    params: [sessionId],
    mapRows: mapNoteAttachmentRows,
  });
  return data ?? [];
}
