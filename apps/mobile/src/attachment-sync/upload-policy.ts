const MAX_RETRY_DELAY_MS = 15 * 60 * 1000;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const UUID_V4_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export function attachmentUploadRetryDelayMs(
  attemptCount: number,
  status?: number,
): number {
  const base = status === 403 ? 5 * 60 * 1000 : 5_000;
  return Math.min(
    MAX_RETRY_DELAY_MS,
    base * 2 ** Math.min(Math.max(attemptCount - 1, 0), 8),
  );
}

export function isPermanentNativeUploadMessage(message: string): boolean {
  return /\b(?:checksum|cipher|integrity|format|invalid|mismatch|path|source)\b/i.test(
    message,
  );
}

export function isPrivateAttachmentIdentity(
  objectId: unknown,
  objectKey: unknown,
): objectId is string {
  if (
    typeof objectId !== "string" ||
    typeof objectKey !== "string" ||
    !UUID_V4_PATTERN.test(objectId)
  ) {
    return false;
  }
  const [ownerId, filename, extra] = objectKey.split("/");
  return (
    extra === undefined &&
    UUID_PATTERN.test(ownerId ?? "") &&
    filename === `${objectId}.anb1`
  );
}
