import type { SupabaseClient } from "@supabase/supabase-js";
import { createClient } from "@supabase/supabase-js";

import { env, requireEnv } from "@/env";

const BUCKET_NAME = "blog";

export interface MediaItem {
  name: string;
  path: string;
  publicUrl: string;
  id: string;
  size: number;
  type: "file" | "dir";
  mimeType: string | null;
  createdAt: string | null;
  updatedAt: string | null;
}

function getSupabaseClient() {
  const key =
    env.SUPABASE_SERVICE_ROLE_KEY ||
    requireEnv(env.SUPABASE_ANON_KEY, "SUPABASE_ANON_KEY");
  return createClient(requireEnv(env.SUPABASE_URL, "SUPABASE_URL"), key);
}

function getPublicUrl(path: string): string {
  const supabase = getSupabaseClient();
  const { data } = supabase.storage.from(BUCKET_NAME).getPublicUrl(path);
  return data.publicUrl;
}

export async function listMediaFiles(
  path: string = "",
): Promise<{ items: MediaItem[]; error?: string }> {
  const supabase = getSupabaseClient();

  try {
    const { data, error } = await supabase.storage
      .from(BUCKET_NAME)
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

    const items: MediaItem[] = data
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
          publicUrl: isFolder ? "" : getPublicUrl(fullPath),
          id: item.id || "",
          size: item.metadata?.size || 0,
          type: isFolder ? "dir" : "file",
          mimeType: item.metadata?.mimetype || null,
          createdAt: item.created_at || null,
          updatedAt: item.updated_at || null,
        };
      });

    const folders = items.filter((item) => item.type === "dir");
    const files = items.filter((item) => item.type === "file");
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
  const allowedExtensions = [
    "jpg",
    "jpeg",
    "png",
    "gif",
    "svg",
    "webp",
    "avif",
    "mp4",
    "webm",
    "mov",
  ];

  const parts = filename.split(".");
  const ext = parts.pop()?.toLowerCase();
  const baseName = parts.join(".").replace(/[^a-zA-Z0-9.-]/g, "-") || "file";

  if (!ext || !allowedExtensions.includes(ext)) {
    return {
      success: false,
      error: "Invalid file type. Only images and videos are allowed.",
    };
  }

  let finalFilename = `${baseName}.${ext}`;
  let path = folder ? `${folder}/${finalFilename}` : finalFilename;

  const { data: existingFiles } = await supabase.storage
    .from(BUCKET_NAME)
    .list(folder || undefined, { limit: 1000 });

  if (existingFiles) {
    const existingNames = new Set(existingFiles.map((f) => f.name));
    let counter = 1;

    while (existingNames.has(finalFilename)) {
      finalFilename = `${baseName}-${counter}.${ext}`;
      counter++;
    }

    path = folder ? `${folder}/${finalFilename}` : finalFilename;
  }

  try {
    const fileBuffer = Buffer.from(content, "base64");

    const mimeTypes: Record<string, string> = {
      jpg: "image/jpeg",
      jpeg: "image/jpeg",
      png: "image/png",
      gif: "image/gif",
      svg: "image/svg+xml",
      webp: "image/webp",
      avif: "image/avif",
      mp4: "video/mp4",
      webm: "video/webm",
      mov: "video/quicktime",
    };

    const { error } = await supabase.storage
      .from(BUCKET_NAME)
      .upload(path, fileBuffer, {
        contentType: mimeTypes[ext] || "application/octet-stream",
        upsert: false,
      });

    if (error) {
      return { success: false, error: error.message };
    }

    const { data } = supabase.storage.from(BUCKET_NAME).getPublicUrl(path);
    return {
      success: true,
      path,
      publicUrl: data.publicUrl,
    };
  } catch (error) {
    return {
      success: false,
      error: `Upload failed: ${(error as Error).message}`,
    };
  }
}

async function listAllFilesInFolder(
  supabase: SupabaseClient,
  folderPath: string,
): Promise<string[]> {
  const allFiles: string[] = [];

  const { data } = await supabase.storage
    .from(BUCKET_NAME)
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
        .from(BUCKET_NAME)
        .list(path, { limit: 1 });

      const isFolder = folderContents && folderContents.length > 0;

      if (isFolder) {
        const allFiles = await listAllFilesInFolder(supabase, path);

        if (allFiles.length > 0) {
          const { error } = await supabase.storage
            .from(BUCKET_NAME)
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
          .from(BUCKET_NAME)
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
      .from(BUCKET_NAME)
      .list(folderPath, { limit: 1 });

    if (existing && existing.length > 0) {
      return { success: false, error: "Folder already exists" };
    }

    const { error } = await supabase.storage
      .from(BUCKET_NAME)
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
      .from(BUCKET_NAME)
      .list(fromPath, { limit: 1 });

    const isFolder = folderContents && folderContents.length > 0;

    if (isFolder) {
      const allFiles = await listAllFilesInFolder(supabase, fromPath);
      const movedFiles: Array<{ from: string; to: string }> = [];

      for (const filePath of allFiles) {
        const relativePath = filePath.substring(fromPath.length);
        const newFilePath = toPath + relativePath;

        const { error } = await supabase.storage
          .from(BUCKET_NAME)
          .move(filePath, newFilePath);

        if (error) {
          if (movedFiles.length > 0) {
            for (const moved of movedFiles) {
              const { error: rollbackError } = await supabase.storage
                .from(BUCKET_NAME)
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
        .from(BUCKET_NAME)
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
