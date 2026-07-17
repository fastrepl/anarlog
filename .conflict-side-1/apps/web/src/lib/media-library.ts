export type MediaProvider = "supabase" | "mux";

export type MediaKind = "image" | "video" | "audio" | "file";

export interface MediaItem {
  name: string;
  path: string;
  publicUrl: string;
  proxyUrl: string;
  id: string;
  size: number;
  type: "file" | "dir";
  mimeType: string | null;
  createdAt: string | null;
  updatedAt: string | null;
  provider?: MediaProvider;
  kind?: MediaKind;
  status?: string | null;
  playbackId?: string | null;
  thumbnailUrl?: string | null;
}

export function getMediaKind(mimeType: string | null | undefined): MediaKind {
  if (mimeType?.startsWith("image/")) return "image";
  if (mimeType?.startsWith("video/")) return "video";
  if (mimeType?.startsWith("audio/")) return "audio";
  return "file";
}

export function getMediaNameFromPath(path: string): string {
  return path.split("/").pop() || path;
}

export function getMediaFolderFromPath(path: string): string {
  const parts = path.split("/");
  parts.pop();
  return parts.join("/");
}
