import {
  type DocumentPickerAsset,
  getDocumentAsync,
} from "expo-document-picker";
import { Directory, File, Paths } from "expo-file-system";

import { catalogSessionAudio } from "@/data/audio-catalog";
import { createSession, deleteSession } from "@/data/session";
import { nowIso } from "@/lib/ids";

const CONTENT_TYPES: Record<string, string> = {
  m4a: "audio/mp4",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
};

export function sessionAudioDirectory(sessionId: string): string {
  return new Directory(Paths.document, "sessions", sessionId).uri;
}

export function recordingDestination(
  sessionId: string,
  extension: string,
): string {
  return new File(Paths.document, "sessions", sessionId, `audio.${extension}`)
    .uri;
}

function splitName(name: string): { title: string; extension: string } {
  const dotIndex = name.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === name.length - 1) {
    return { title: name, extension: "m4a" };
  }
  return {
    title: name.slice(0, dotIndex),
    extension: name.slice(dotIndex + 1).toLowerCase(),
  };
}

async function importAsset(asset: DocumentPickerAsset): Promise<string> {
  const { title, extension } = splitName(asset.name);
  const createdAt =
    typeof asset.lastModified === "number" && asset.lastModified > 0
      ? new Date(asset.lastModified).toISOString()
      : nowIso();

  const sessionId = await createSession({ title, createdAt });

  try {
    const directory = new Directory(Paths.document, "sessions", sessionId);
    directory.create({ intermediates: true, idempotent: true });

    const filename = `audio.${extension}`;
    const destination = new File(directory, filename);
    await new File(asset.uri).copy(destination);

    await catalogSessionAudio(sessionId, {
      filename,
      contentType:
        asset.mimeType ??
        CONTENT_TYPES[extension] ??
        "application/octet-stream",
      sizeBytes: asset.size ?? destination.size ?? 0,
    });
  } catch (error) {
    // The session exists before the audio does, so a failed copy or catalog
    // would otherwise strand an empty session on the timeline.
    await deleteSession(sessionId).catch(() => {});
    throw error;
  }

  return sessionId;
}

export async function importVoiceMemos(): Promise<string[]> {
  const result = await getDocumentAsync({
    type: ["audio/*"],
    multiple: true,
    copyToCacheDirectory: true,
  });
  if (result.canceled) return [];

  const created: string[] = [];
  for (const asset of result.assets) {
    try {
      created.push(await importAsset(asset));
    } catch (error) {
      console.warn(`voice memo import failed for ${asset.name}`, error);
    }
  }
  return created;
}
