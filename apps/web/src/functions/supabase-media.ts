import type { SupabaseClient } from "@supabase/supabase-js";
import { createClient } from "@supabase/supabase-js";

import { env, requireEnv } from "@/env";
import { listCatalogMediaItems } from "@/functions/media-catalog";
import {
  getMimeTypeFromExtension,
  MEDIA_BUCKET_NAME,
  parseMediaFilename,
} from "@/lib/media";
import type { MediaItem } from "@/lib/media-library";

const MEDIA_LIST_CACHE_TTL_MS = 30_000;

type MediaListResult = { items: MediaItem[]; error?: string };

const mediaListCache = new Map<
  string,
  { expiresAt: number; result: MediaListResult }
>();
const mediaListInFlight = new Map<string, Promise<MediaListResult>>();
let mediaListCacheVersion = 0;

function getSupabaseClient() {
  const key =
    env.SUPABASE_SERVICE_ROLE_KEY ||
    requireEnv(env.SUPABASE_ANON_KEY, "SUPABASE_ANON_KEY");
  return createClient(requireEnv(env.SUPABASE_URL, "SUPABASE_URL"), key);
}

function getPublicUrl(supabase: SupabaseClient, path: string): string {
  const { data } = supabase.storage.from(MEDIA_BUCKET_NAME).getPublicUrl(path);
  return data.publicUrl;
}

function normalizePath(path: string): string {
  return path.split("/").filter(Boolean).join("/");
}

function arePathsRelated(a: string, b: string): boolean {
  if (a === "" || b === "") return true;
  return a === b || a.startsWith(`${b}/`) || b.startsWith(`${a}/`);
}

export function invalidateMediaListCache(paths: string[]) {
  if (paths.length === 0) return;

  const targets = [...new Set(paths.map((path) => normalizePath(path)))];
  mediaListCacheVersion += 1;

  if (targets.includes("")) {
    mediaListCache.clear();
    mediaListInFlight.clear();
    return;
  }

  for (const key of [...mediaListCache.keys()]) {
    if (targets.some((target) => arePathsRelated(key, target))) {
      mediaListCache.delete(key);
    }
  }

  for (const key of [...mediaListInFlight.keys()]) {
    if (targets.some((target) => arePathsRelated(key, target))) {
      mediaListInFlight.delete(key);
    }
  }
}

async function resolveUploadPath(
  supabase: SupabaseClient,
  params: {
    filename?: string;
    folder?: string;
    path?: string;
    upsert?: boolean;
  },
): Promise<
  | {
      success: true;
      path: string;
      contentType: string;
    }
  | {
      success: false;
      error: string;
    }
> {
  if (params.path) {
    const path = normalizePath(params.path);
    const filename = path.split("/").pop();
    if (!filename) {
      return {
        success: false,
        error: "Invalid file path",
      };
    }

    const parsed = parseMediaFilename(filename);
    if (!parsed) {
      return {
        success: false,
        error: "Invalid file type. Only images and videos are allowed.",
      };
    }

    return {
      success: true,
      path,
      contentType: getMimeTypeFromExtension(parsed.extension),
    };
  }

  if (!params.filename) {
    return {
      success: false,
      error: "Missing filename",
    };
  }

  const parsed = parseMediaFilename(params.filename);
  if (!parsed) {
    return {
      success: false,
      error: "Invalid file type. Only images and videos are allowed.",
    };
  }

  const folder = normalizePath(params.folder || "");
  let filename = parsed.filename;
  let path = folder ? `${folder}/${filename}` : filename;

  if (!params.upsert) {
    const { data: existingFiles } = await supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .list(folder || undefined, { limit: 1000 });

    if (existingFiles) {
      const existingNames = new Set(existingFiles.map((file) => file.name));
      let counter = 1;

      while (existingNames.has(filename)) {
        filename = `${parsed.baseName}-${counter}.${parsed.extension}`;
        counter++;
      }

      path = folder ? `${folder}/${filename}` : filename;
    }
  }

  return {
    success: true,
    path,
    contentType: getMimeTypeFromExtension(parsed.extension),
  };
}

async function loadMediaFiles(path: string): Promise<MediaListResult> {
  const supabase = getSupabaseClient();

  try {
    const { data, error } = await supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .list(path, {
        limit: 1000,
        sortBy: { column: "name", order: "asc" },
      });

    if (error) {
      return { items: [], error: error.message };
    }

    if (!data) {
      return { items: [] };
    }

    const storageItems: MediaItem[] = data
      .filter(
        (item) =>
          item.name !== ".emptyFolderPlaceholder" && item.name !== ".folder",
      )
      .map((item) => {
        const fullPath = path ? `${path}/${item.name}` : item.name;
        const isFolder = item.id === null;

        return {
          name: item.name,
          path: fullPath,
          publicUrl: isFolder ? "" : getPublicUrl(supabase, fullPath),
          id: item.id || "",
          size: item.metadata?.size || 0,
          type: isFolder ? "dir" : "file",
          mimeType: item.metadata?.mimetype || null,
          createdAt: item.created_at || null,
          updatedAt: item.updated_at || null,
        };
      });

    const folders = storageItems.filter((item) => item.type === "dir");
    const storageFiles = storageItems.filter((item) => item.type === "file");
    const catalog = await listCatalogMediaItems(supabase, path);
    const fileMap = new Map(
      (catalog.supported ? catalog.items : []).map((item) => [item.path, item]),
    );

    for (const file of storageFiles) {
      if (!fileMap.has(file.path)) {
        fileMap.set(file.path, file);
      }
    }

    const files = [...fileMap.values()];

    folders.sort((a, b) => a.name.localeCompare(b.name));
    files.sort((a, b) => a.name.localeCompare(b.name));

    return { items: [...folders, ...files] };
  } catch (error) {
    return {
      items: [],
      error: `Failed to list files: ${(error as Error).message}`,
    };
  }
}

export async function listMediaFiles(
  path: string = "",
): Promise<MediaListResult> {
  const normalizedPath = normalizePath(path);
  const now = Date.now();
  const cached = mediaListCache.get(normalizedPath);

  if (cached && cached.expiresAt > now) {
    return cached.result;
  }

  if (cached) {
    mediaListCache.delete(normalizedPath);
  }

  const existingPromise = mediaListInFlight.get(normalizedPath);
  if (existingPromise) {
    return existingPromise;
  }

  const cacheVersion = mediaListCacheVersion;
  const promise = loadMediaFiles(normalizedPath)
    .then((result) => {
      if (!result.error && cacheVersion === mediaListCacheVersion) {
        mediaListCache.set(normalizedPath, {
          expiresAt: Date.now() + MEDIA_LIST_CACHE_TTL_MS,
          result,
        });
      }

      return result;
    })
    .finally(() => {
      mediaListInFlight.delete(normalizedPath);
    });

  mediaListInFlight.set(normalizedPath, promise);
  return promise;
}

export async function uploadMediaFile(
  supabase: SupabaseClient,
  filename: string,
  content: string,
  folder: string = "",
): Promise<{
  success: boolean;
  path?: string;
  publicUrl?: string;
  error?: string;
}> {
  const resolvedPath = await resolveUploadPath(supabase, { filename, folder });
  if (!resolvedPath.success) {
    return {
      success: false,
      error: resolvedPath.error,
    };
  }

  try {
    const fileBuffer = Buffer.from(content, "base64");

    const { error } = await supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .upload(resolvedPath.path, fileBuffer, {
        contentType: resolvedPath.contentType,
        upsert: false,
      });

    if (error) {
      return { success: false, error: error.message };
    }

    const { data } = supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .getPublicUrl(resolvedPath.path);
    return {
      success: true,
      path: resolvedPath.path,
      publicUrl: data.publicUrl,
    };
  } catch (error) {
    return {
      success: false,
      error: `Upload failed: ${(error as Error).message}`,
    };
  }
}

export async function createSignedMediaUpload(
  supabase: SupabaseClient,
  params: {
    filename?: string;
    folder?: string;
    path?: string;
    upsert?: boolean;
  },
): Promise<{
  success: boolean;
  path?: string;
  publicUrl?: string;
  token?: string;
  signedUrl?: string;
  error?: string;
}> {
  const resolvedPath = await resolveUploadPath(supabase, params);
  if (!resolvedPath.success) {
    return {
      success: false,
      error: resolvedPath.error,
    };
  }

  const { data, error } = await supabase.storage
    .from(MEDIA_BUCKET_NAME)
    .createSignedUploadUrl(resolvedPath.path, {
      upsert: params.upsert ?? false,
    });

  if (error) {
    return {
      success: false,
      error: error.message,
    };
  }

  return {
    success: true,
    path: resolvedPath.path,
    publicUrl: getPublicUrl(supabase, resolvedPath.path),
    token: data.token,
    signedUrl: data.signedUrl,
  };
}

async function listAllFilesInFolder(
  supabase: SupabaseClient,
  folderPath: string,
): Promise<string[]> {
  const allFiles: string[] = [];

  const { data } = await supabase.storage
    .from(MEDIA_BUCKET_NAME)
    .list(folderPath, { limit: 1000 });

  if (!data) return allFiles;

  for (const item of data) {
    const itemPath = folderPath ? `${folderPath}/${item.name}` : item.name;
    const isFolder = item.id === null;

    if (isFolder) {
      const nestedFiles = await listAllFilesInFolder(supabase, itemPath);
      allFiles.push(...nestedFiles);
    } else {
      allFiles.push(itemPath);
    }
  }

  return allFiles;
}

export async function deleteMediaFiles(
  supabase: SupabaseClient,
  paths: string[],
): Promise<{ success: boolean; deleted: string[]; errors: string[] }> {
  const deleted: string[] = [];
  const errors: string[] = [];

  try {
    for (const path of paths) {
      const { data: folderContents } = await supabase.storage
        .from(MEDIA_BUCKET_NAME)
        .list(path, { limit: 1 });

      const isFolder = folderContents && folderContents.length > 0;

      if (isFolder) {
        const allFiles = await listAllFilesInFolder(supabase, path);

        if (allFiles.length > 0) {
          const { error } = await supabase.storage
            .from(MEDIA_BUCKET_NAME)
            .remove(allFiles);

          if (error) {
            errors.push(`${path}: ${error.message}`);
          } else {
            deleted.push(path);
          }
        } else {
          deleted.push(path);
        }
      } else {
        const { data, error } = await supabase.storage
          .from(MEDIA_BUCKET_NAME)
          .remove([path]);

        if (error) {
          errors.push(`${path}: ${error.message}`);
        } else if (data && data.length > 0) {
          deleted.push(path);
        } else {
          errors.push(
            `${path}: File was not deleted - check storage permissions or file path`,
          );
        }
      }
    }

    return {
      success: deleted.length > 0 && errors.length === 0,
      deleted,
      errors,
    };
  } catch (error) {
    return {
      success: false,
      deleted,
      errors: [`Delete failed: ${(error as Error).message}`],
    };
  }
}

export async function createMediaFolder(
  supabase: SupabaseClient,
  folderName: string,
  parentFolder: string = "",
): Promise<{ success: boolean; path?: string; error?: string }> {
  const sanitizedFolderName = folderName
    .replace(/[^a-zA-Z0-9-_]/g, "-")
    .toLowerCase();

  const folderPath = parentFolder
    ? `${parentFolder}/${sanitizedFolderName}`
    : sanitizedFolderName;

  const placeholderPath = `${folderPath}/.emptyFolderPlaceholder`;

  try {
    const { data: existing } = await supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .list(folderPath, { limit: 1 });

    if (existing && existing.length > 0) {
      return { success: false, error: "Folder already exists" };
    }

    const { error } = await supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .upload(placeholderPath, new Uint8Array(0), {
        contentType: "application/x-empty",
        upsert: false,
      });

    if (error) {
      return { success: false, error: error.message };
    }

    return {
      success: true,
      path: folderPath,
    };
  } catch (error) {
    return {
      success: false,
      error: `Failed to create folder: ${(error as Error).message}`,
    };
  }
}

export async function moveMediaFile(
  supabase: SupabaseClient,
  fromPath: string,
  toPath: string,
): Promise<{ success: boolean; newPath?: string; error?: string }> {
  try {
    const { data: folderContents } = await supabase.storage
      .from(MEDIA_BUCKET_NAME)
      .list(fromPath, { limit: 1 });

    const isFolder = folderContents && folderContents.length > 0;

    if (isFolder) {
      const allFiles = await listAllFilesInFolder(supabase, fromPath);
      const movedFiles: Array<{ from: string; to: string }> = [];

      for (const filePath of allFiles) {
        const relativePath = filePath.substring(fromPath.length);
        const newFilePath = toPath + relativePath;

        const { error } = await supabase.storage
          .from(MEDIA_BUCKET_NAME)
          .move(filePath, newFilePath);

        if (error) {
          if (movedFiles.length > 0) {
            for (const moved of movedFiles) {
              const { error: rollbackError } = await supabase.storage
                .from(MEDIA_BUCKET_NAME)
                .move(moved.to, moved.from);
              if (rollbackError) {
                // Log or handle rollback failure
                console.error(
                  `Rollback failed for ${moved.to}: ${rollbackError.message}`,
                );
              }
            }
          }

          return {
            success: false,
            error: `Failed to move ${filePath}: ${error.message}`,
          };
        }

        movedFiles.push({ from: filePath, to: newFilePath });
      }

      return {
        success: true,
        newPath: toPath,
      };
    } else {
      const { error } = await supabase.storage
        .from(MEDIA_BUCKET_NAME)
        .move(fromPath, toPath);

      if (error) {
        return { success: false, error: error.message };
      }

      return {
        success: true,
        newPath: toPath,
      };
    }
  } catch (error) {
    return {
      success: false,
      error: `Move failed: ${(error as Error).message}`,
    };
  }
}
