export type NoteAttachmentRow = {
  id: string;
  source_id: string;
  filename: string;
  relative_path: string;
  content_type: string;
  size_bytes: number;
  sha256: string;
  created_at: string;
  available_locally: number;
  local_relative_path: string;
  cloud_object_key: string;
};

export type NoteAttachment = {
  attachmentId: string;
  sourceId: string;
  filename: string;
  relativePath: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  createdAt: string;
  availableLocally: boolean;
  localRelativePath: string | null;
  cloudObjectKey: string | null;
};

const MAX_ATTACHMENT_BYTES = 512 * 1024 * 1024;
const UUID_V4_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export function mapNoteAttachmentRows(
  rows: NoteAttachmentRow[],
): NoteAttachment[] {
  return rows.flatMap((row) => {
    if (
      !UUID_V4_PATTERN.test(row.id) ||
      !validBasename(row.source_id) ||
      !validBasename(row.filename) ||
      row.relative_path !== `attachments/${row.source_id}` ||
      !Number.isSafeInteger(row.size_bytes) ||
      row.size_bytes <= 0 ||
      row.size_bytes > MAX_ATTACHMENT_BYTES ||
      !SHA256_PATTERN.test(row.sha256) ||
      !validContentType(row.content_type)
    ) {
      return [];
    }
    const availableLocally =
      row.available_locally === 1 &&
      row.local_relative_path === row.relative_path;
    return [
      {
        attachmentId: row.id,
        sourceId: row.source_id,
        filename: row.filename,
        relativePath: row.relative_path,
        contentType: row.content_type,
        sizeBytes: row.size_bytes,
        sha256: row.sha256,
        createdAt: row.created_at,
        availableLocally,
        localRelativePath: availableLocally ? row.local_relative_path : null,
        cloudObjectKey: row.cloud_object_key || null,
      },
    ];
  });
}

function validBasename(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 1024 &&
    value.trim() === value &&
    !value.includes("/") &&
    !value.includes("\\") &&
    value !== "." &&
    value !== ".." &&
    !hasControlCharacter(value)
  );
}

function validContentType(value: string): boolean {
  return (
    value.length <= 512 && value.trim() === value && !hasControlCharacter(value)
  );
}

function hasControlCharacter(value: string): boolean {
  return [...value].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 31 || code === 127;
  });
}
