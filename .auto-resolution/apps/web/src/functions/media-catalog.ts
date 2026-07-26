import type { SupabaseClient } from "@supabase/supabase-js";

import { getMediaProxyUrl, MEDIA_BUCKET_NAME } from "@/lib/media";
import {
  getMediaFolderFromPath,
  getMediaKind,
  getMediaNameFromPath,
  type MediaItem,
} from "@/lib/media-library";

const MEDIA_CATALOG_TABLE = "media_assets";

type MediaAssetRow = {
  id: string;
  provider: "supabase" | "mux";
  kind: string | null;
  name: string | null;
  folder: string | null;
  library_path: string;
  public_url: string | null;
  mime_type: string | null;
  size: number | string | null;
  status: string | null;
  storage_path: string | null;
  mux_playback_id: string | null;
  thumbnail_url: string | null;
  created_at: string | null;
  updated_at: string | null;
};

function isCatalogUnavailableError(
  error: {
    code?: string | null;
    message?: string | null;
  } | null,
) {
  if (!error) return false;

  return (
    error.code === "42P01" ||
    error.code === "PGRST204" ||
    error.code === "PGRST205" ||
    error.message?.includes("media_assets") === true
  );
}

function getMuxThumbnailUrl(playbackId: string | null | undefined) {
  if (!playbackId) return null;
  return `https://image.mux.com/${playbackId}/thumbnail.jpg?width=640&height=360&fit_mode=smartcrop`;
}

function mapCatalogRowToMediaItem(row: MediaAssetRow): MediaItem {
  return {
    name: row.name || getMediaNameFromPath(row.library_path),
    path: row.library_path,
    publicUrl: row.public_url || "",
    proxyUrl:
      row.provider === "supabase"
        ? getMediaProxyUrl(row.library_path)
        : row.public_url || "",
    id: row.id,
    size:
      typeof row.size === "number"
        ? row.size
        : Number.parseInt(row.size || "0", 10) || 0,
    type: "file",
    mimeType: row.mime_type,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    provider: row.provider,
    kind:
      row.kind === "image" ||
      row.kind === "video" ||
      row.kind === "audio" ||
      row.kind === "file"
        ? row.kind
        : getMediaKind(row.mime_type),
    status: row.status,
    playbackId: row.mux_playback_id,
    thumbnailUrl:
      row.thumbnail_url || getMuxThumbnailUrl(row.mux_playback_id) || null,
  };
}

export async function listCatalogMediaItems(
  supabase: SupabaseClient,
  folder: string,
): Promise<{ supported: boolean; items: MediaItem[] }> {
  const { data, error } = await supabase
    .from(MEDIA_CATALOG_TABLE)
    .select(
      "id, provider, kind, name, folder, library_path, public_url, mime_type, size, status, storage_path, mux_playback_id, thumbnail_url, created_at, updated_at",
    )
    .eq("folder", folder)
    .order("name", { ascending: true });

  if (error) {
    if (isCatalogUnavailableError(error)) {
      return { supported: false, items: [] };
    }

    throw new Error(error.message);
  }

  return {
    supported: true,
    items: ((data || []) as MediaAssetRow[]).map(mapCatalogRowToMediaItem),
  };
}

export async function registerStorageMediaAsset(
  supabase: SupabaseClient,
  params: {
    path: string;
    publicUrl: string;
    mimeType: string | null;
    size: number;
  },
) {
  const now = new Date().toISOString();
  const payload = {
    provider: "supabase",
    kind: getMediaKind(params.mimeType),
    name: getMediaNameFromPath(params.path),
    folder: getMediaFolderFromPath(params.path),
    library_path: params.path,
    public_url: params.publicUrl,
    mime_type: params.mimeType,
    size: params.size,
    status: "ready",
    storage_path: params.path,
    mux_playback_id: null,
    thumbnail_url: null,
    updated_at: now,
  };

  const { error } = await supabase
    .from(MEDIA_CATALOG_TABLE)
    .upsert(payload, { onConflict: "library_path" });

  if (error && !isCatalogUnavailableError(error)) {
    throw new Error(error.message);
  }
}

export async function deleteCatalogMediaAssets(
  supabase: SupabaseClient,
  paths: string[],
) {
  for (const path of paths) {
    const { error: exactError } = await supabase
      .from(MEDIA_CATALOG_TABLE)
      .delete()
      .eq("library_path", path);

    if (exactError && !isCatalogUnavailableError(exactError)) {
      throw new Error(exactError.message);
    }

    const { error: nestedError } = await supabase
      .from(MEDIA_CATALOG_TABLE)
      .delete()
      .like("library_path", `${path}/%`);

    if (nestedError && !isCatalogUnavailableError(nestedError)) {
      throw new Error(nestedError.message);
    }
  }
}

export async function moveCatalogMediaAsset(
  supabase: SupabaseClient,
  fromPath: string,
  toPath: string,
) {
  const exactPromise = supabase
    .from(MEDIA_CATALOG_TABLE)
    .select(
      "id, provider, kind, name, folder, library_path, public_url, mime_type, size, status, storage_path, mux_playback_id, thumbnail_url, created_at, updated_at",
    )
    .eq("library_path", fromPath);
  const nestedPromise = supabase
    .from(MEDIA_CATALOG_TABLE)
    .select(
      "id, provider, kind, name, folder, library_path, public_url, mime_type, size, status, storage_path, mux_playback_id, thumbnail_url, created_at, updated_at",
    )
    .like("library_path", `${fromPath}/%`);

  const [
    { data: exactData, error: exactError },
    { data: nestedData, error: nestedError },
  ] = await Promise.all([exactPromise, nestedPromise]);

  if (exactError && !isCatalogUnavailableError(exactError)) {
    throw new Error(exactError.message);
  }

  if (nestedError && !isCatalogUnavailableError(nestedError)) {
    throw new Error(nestedError.message);
  }

  if (
    isCatalogUnavailableError(exactError) ||
    isCatalogUnavailableError(nestedError)
  ) {
    return;
  }

  const rows = [...(exactData || []), ...(nestedData || [])] as MediaAssetRow[];
  const seen = new Set<string>();
  const now = new Date().toISOString();

  for (const row of rows) {
    if (seen.has(row.id)) continue;
    seen.add(row.id);

    const nextLibraryPath =
      row.library_path === fromPath
        ? toPath
        : `${toPath}${row.library_path.slice(fromPath.length)}`;
    const nextStoragePath =
      row.provider === "supabase" && row.storage_path
        ? row.storage_path === fromPath
          ? toPath
          : `${toPath}${row.storage_path.slice(fromPath.length)}`
        : row.storage_path;
    const nextPublicUrl =
      row.provider === "supabase" && nextStoragePath
        ? supabase.storage.from(MEDIA_BUCKET_NAME).getPublicUrl(nextStoragePath)
            .data.publicUrl
        : row.public_url;

    const { error } = await supabase
      .from(MEDIA_CATALOG_TABLE)
      .update({
        name: getMediaNameFromPath(nextLibraryPath),
        folder: getMediaFolderFromPath(nextLibraryPath),
        library_path: nextLibraryPath,
        storage_path: nextStoragePath,
        public_url: nextPublicUrl,
        updated_at: now,
      })
      .eq("id", row.id);

    if (error && !isCatalogUnavailableError(error)) {
      throw new Error(error.message);
    }
  }
}
