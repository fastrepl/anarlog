import type { AudioSourceMetadata } from "@hypr/plugin-fs-sync";

export function estimateUploadedAudioSessionCreatedAt(
  metadata: Pick<
    AudioSourceMetadata,
    "createdAt" | "modifiedAt" | "durationMs"
  >,
): string | null {
  const anchor = metadata.createdAt ?? metadata.modifiedAt;
  if (!anchor) {
    return null;
  }

  const anchorMs = Date.parse(anchor);
  if (Number.isNaN(anchorMs)) {
    return null;
  }

  const durationMs =
    typeof metadata.durationMs === "number" && metadata.durationMs > 0
      ? metadata.durationMs
      : 0;

  return new Date(Math.max(0, anchorMs - durationMs)).toISOString();
}
