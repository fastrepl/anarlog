import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  CircleIcon,
  Loader2Icon,
  XCircleIcon,
} from "lucide-react";

import { sonnerToast as toast } from "@hypr/ui/components/ui/toast";

type FileStatus = "pending" | "uploading" | "done" | "error";

interface FileProgress {
  name: string;
  status: FileStatus;
}

function UploadToast({
  files,
  done,
  error,
}: {
  files: FileProgress[];
  done?: boolean;
  error?: string;
}) {
  const completedCount = files.filter((f) => f.status === "done").length;

  return (
    <div className="flex flex-col gap-2 p-3">
      <div className="text-sm font-medium">
        {error
          ? "Upload failed"
          : done
            ? `Uploaded ${completedCount} file${completedCount !== 1 ? "s" : ""}`
            : `Uploading ${files.length} file${files.length !== 1 ? "s" : ""}...`}
      </div>
      <div className="flex max-h-32 flex-col gap-1 overflow-y-auto">
        {files.map((file, i) => (
          <div key={i} className="flex items-center gap-2 text-xs">
            {file.status === "pending" && (
              <CircleIcon className="size-3 shrink-0 text-neutral-300" />
            )}
            {file.status === "uploading" && (
              <Loader2Icon className="size-3 shrink-0 animate-spin text-blue-500" />
            )}
            {file.status === "done" && (
              <CheckCircle2Icon className="size-3 shrink-0 text-green-500" />
            )}
            {file.status === "error" && (
              <XCircleIcon className="size-3 shrink-0 text-red-500" />
            )}
            <span
              className={[
                "truncate",
                file.status === "done"
                  ? "text-neutral-400"
                  : "text-neutral-600",
              ].join(" ")}
            >
              {file.name}
            </span>
          </div>
        ))}
      </div>
      {error && <div className="text-xs text-red-500"> {error} </div>}
    </div>
  );
}

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

export async function fetchMediaItems(path: string): Promise<MediaItem[]> {
  const response = await fetch(
    `/api/admin/media/list?path=${encodeURIComponent(path)}`,
  );
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data.error || "Failed to fetch media");
  }
  return data.items;
}

async function uploadFile(params: {
  filename: string;
  content: string;
  folder: string;
}) {
  const response = await fetch("/api/admin/media/upload", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(params),
  });

  if (!response.ok) {
    const data = await response.json();
    throw new Error(data.error || "Upload failed");
  }
  return response.json();
}

async function deleteFiles(paths: string[]) {
  const response = await fetch("/api/admin/media/delete", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ paths }),
  });

  const data = await response.json();
  if (data.errors && data.errors.length > 0) {
    throw new Error(`Some files failed to delete: ${data.errors.join(", ")}`);
  }
  return data;
}

async function createFolder(params: { name: string; parentFolder: string }) {
  const response = await fetch("/api/admin/media/create-folder", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(params),
  });

  if (!response.ok) {
    const data = await response.json();
    throw new Error(data.error || "Failed to create folder");
  }
  return response.json();
}

async function moveFile(params: { fromPath: string; toPath: string }) {
  const response = await fetch("/api/admin/media/move", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(params),
  });

  if (!response.ok) {
    const data = await response.json();
    throw new Error(data.error || "Failed to move file");
  }
  return response.json();
}

export function useMediaApi({
  currentFolderPath,
  onFolderCreated,
  onFileMoved,
  onSelectionCleared,
}: {
  currentFolderPath: string;
  onFolderCreated?: (parentFolder: string) => void;
  onFileMoved?: () => void;
  onSelectionCleared?: () => void;
}) {
  const queryClient = useQueryClient();

  const invalidateAndRefresh = () => {
    queryClient.invalidateQueries({ queryKey: ["mediaItems"] });
  };

  const uploadMutation = useMutation({
    mutationFn: async (files: FileList) => {
      const fileArray = Array.from(files);
      const fileProgress: FileProgress[] = fileArray.map((f) => ({
        name: f.name,
        status: "pending",
      }));

      const toastId = "upload-toast";
      const updateToast = (done?: boolean, error?: string) => {
        toast.custom(
          () => (
            <UploadToast files={[...fileProgress]} done={done} error={error} />
          ),
          {
            id: toastId,
            duration: done || error ? 3000 : Infinity,
          },
        );
      };

      updateToast();

      try {
        for (let i = 0; i < fileArray.length; i++) {
          const file = fileArray[i];
          fileProgress[i].status = "uploading";
          updateToast();

          const reader = new FileReader();
          const content = await new Promise<string>((resolve, reject) => {
            reader.onload = () => {
              const base64 = (reader.result as string).split(",")[1];
              resolve(base64);
            };
            reader.onerror = reject;
            reader.readAsDataURL(file);
          });

          await uploadFile({
            filename: file.name,
            content,
            folder: currentFolderPath,
          });

          fileProgress[i].status = "done";
          updateToast();
        }

        updateToast(true);
      } catch (error) {
        const currentIndex = fileProgress.findIndex(
          (f) => f.status === "uploading",
        );
        if (currentIndex !== -1) {
          fileProgress[currentIndex].status = "error";
        }
        updateToast(
          false,
          error instanceof Error ? error.message : "Unknown error",
        );
        throw error;
      }
    },
    onSuccess: () => {
      invalidateAndRefresh();
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (paths: string[]) => deleteFiles(paths),
    onSuccess: () => {
      onSelectionCleared?.();
      invalidateAndRefresh();
    },
  });

  const replaceMutation = useMutation({
    mutationFn: async (params: { file: File; path: string }) => {
      const reader = new FileReader();
      const content = await new Promise<string>((resolve, reject) => {
        reader.onload = () => {
          const base64 = (reader.result as string).split(",")[1];
          resolve(base64);
        };
        reader.onerror = reject;
        reader.readAsDataURL(params.file);
      });

      await uploadFile({
        filename: params.file.name,
        content,
        folder: params.path.split("/").slice(0, -1).join("/"),
      });
    },
    onSuccess: () => {
      invalidateAndRefresh();
    },
  });

  const createFolderMutation = useMutation({
    mutationFn: (params: { name: string; parentFolder: string }) =>
      createFolder(params),
    onSuccess: (_, variables) => {
      invalidateAndRefresh();
      onFolderCreated?.(variables.parentFolder);
    },
  });

  const moveMutation = useMutation({
    mutationFn: (params: { fromPath: string; toPath: string }) =>
      moveFile(params),
    onSuccess: () => {
      invalidateAndRefresh();
      onFileMoved?.();
    },
  });

  const renameMutation = useMutation({
    mutationFn: (params: { path: string; newName: string }) => {
      const parts = params.path.split("/");
      parts[parts.length - 1] = params.newName;
      const newPath = parts.join("/");
      return moveFile({ fromPath: params.path, toPath: newPath });
    },
    onSuccess: () => {
      invalidateAndRefresh();
    },
  });

  return {
    uploadMutation,
    deleteMutation,
    replaceMutation,
    createFolderMutation,
    moveMutation,
    renameMutation,
  };
}
