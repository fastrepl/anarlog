import {
  type QueryClient,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  CircleIcon,
  Loader2Icon,
  XCircleIcon,
} from "lucide-react";

import { sonnerToast as toast } from "@hypr/ui/components/ui/toast";

import { uploadMediaLibraryFile } from "@/functions/media-upload";
import { fetchAdminJson, isAdminSignInRedirectError } from "@/lib/admin-auth";
import { getMediaFolderFromPath, type MediaItem } from "@/lib/media-library";

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

export type { MediaItem } from "@/lib/media-library";

const MEDIA_ITEMS_QUERY_STALE_TIME = 30_000;
const MEDIA_ITEMS_QUERY_GC_TIME = 5 * 60_000;

export function getMediaItemsQueryKey(path: string) {
  return ["mediaItems", path] as const;
}

export async function fetchMediaItems(path: string): Promise<MediaItem[]> {
  const data = await fetchAdminJson<{ items: MediaItem[] }>(
    `/api/admin/media/list?path=${encodeURIComponent(path)}`,
    undefined,
    "Failed to fetch media",
  );

  return data.items;
}

export function getMediaItemsQueryOptions(path: string) {
  return {
    queryKey: getMediaItemsQueryKey(path),
    queryFn: () => fetchMediaItems(path),
    staleTime: MEDIA_ITEMS_QUERY_STALE_TIME,
    gcTime: MEDIA_ITEMS_QUERY_GC_TIME,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  };
}

function invalidateMediaItemPaths(queryClient: QueryClient, paths: string[]) {
  const uniquePaths = [...new Set(paths)];

  return Promise.all(
    uniquePaths.map((path) =>
      queryClient.invalidateQueries({
        queryKey: getMediaItemsQueryKey(path),
      }),
    ),
  );
}

async function deleteFiles(paths: string[]) {
  const data = await fetchAdminJson<{
    errors?: string[];
  }>(
    "/api/admin/media/delete",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ paths }),
    },
    "Failed to delete files",
  );

  if (data.errors && data.errors.length > 0) {
    throw new Error(`Some files failed to delete: ${data.errors.join(", ")}`);
  }
  return data;
}

async function createFolder(params: { name: string; parentFolder: string }) {
  return fetchAdminJson<any>(
    "/api/admin/media/create-folder",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(params),
    },
    "Failed to create folder",
  );
}

async function moveFile(params: { fromPath: string; toPath: string }) {
  return fetchAdminJson<any>(
    "/api/admin/media/move",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(params),
    },
    "Failed to move file",
  );
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

          await uploadMediaLibraryFile({
            file,
            folder: currentFolderPath,
          });

          fileProgress[i].status = "done";
          updateToast();
        }

        updateToast(true);
      } catch (error) {
        if (isAdminSignInRedirectError(error)) {
          throw error;
        }

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
      return invalidateMediaItemPaths(queryClient, [currentFolderPath]);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (paths: string[]) => deleteFiles(paths),
    onSuccess: () => {
      onSelectionCleared?.();
      return invalidateMediaItemPaths(queryClient, [currentFolderPath]);
    },
  });

  const replaceMutation = useMutation({
    mutationFn: async (params: { file: File; path: string }) => {
      await uploadMediaLibraryFile({
        file: params.file,
        path: params.path,
        upsert: true,
      });
    },
    onSuccess: () => {
      return invalidateMediaItemPaths(queryClient, [currentFolderPath]);
    },
  });

  const createFolderMutation = useMutation({
    mutationFn: (params: { name: string; parentFolder: string }) =>
      createFolder(params),
    onSuccess: (_, variables) => {
      void invalidateMediaItemPaths(queryClient, [variables.parentFolder]);
      onFolderCreated?.(variables.parentFolder);
    },
  });

  const moveMutation = useMutation({
    mutationFn: (params: { fromPath: string; toPath: string }) =>
      moveFile(params),
    onSuccess: (_, variables) => {
      void invalidateMediaItemPaths(queryClient, [
        getMediaFolderFromPath(variables.fromPath),
        getMediaFolderFromPath(variables.toPath),
      ]);
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
    onSuccess: (_, variables) => {
      return invalidateMediaItemPaths(queryClient, [
        getMediaFolderFromPath(variables.path),
      ]);
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
