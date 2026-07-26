type PendingUpload = { kind: "audio" | "transcript"; filePath: string };

const pending = new Map<string, PendingUpload>();

export function setPendingUpload(
  sessionId: string,
  upload: PendingUpload,
): void {
  pending.set(sessionId, upload);
}

export function consumePendingUpload(sessionId: string): PendingUpload | null {
  const upload = pending.get(sessionId);
  if (upload) {
    pending.delete(sessionId);
    return upload;
  }
  return null;
}
