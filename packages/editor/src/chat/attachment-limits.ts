export const MAX_CHAT_IMAGE_BYTES = 8 * 1024 * 1024;
export const MAX_CHAT_DRAFT_BYTES = 16 * 1024 * 1024;

export const CHAT_ATTACHMENT_OVERHEAD_BYTES = 512;

export function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

export function estimateImageDataUrlBytes(size: number, mimeType: string) {
  const prefixBytes = utf8Length(`data:${mimeType};base64,`);
  return prefixBytes + Math.ceil(size / 3) * 4;
}

export function canRetainChatImage({
  fileSize,
  mimeType,
  currentDraftBytes,
  pendingImageBytes,
}: {
  fileSize: number;
  mimeType: string;
  currentDraftBytes: number;
  pendingImageBytes: number;
}) {
  if (fileSize > MAX_CHAT_IMAGE_BYTES) {
    return false;
  }

  return (
    currentDraftBytes +
      pendingImageBytes +
      estimateImageDataUrlBytes(fileSize, mimeType) +
      CHAT_ATTACHMENT_OVERHEAD_BYTES <=
    MAX_CHAT_DRAFT_BYTES
  );
}
