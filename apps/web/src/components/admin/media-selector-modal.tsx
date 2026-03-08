import { Icon } from "@iconify-icon/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

interface MediaItem {
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

async function fetchMediaItems(path: string): Promise<MediaItem[]> {
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

function getRelativePath(fullPath: string): string {
  return fullPath.replace(/^apps\/web\/public\/images\/?/, "");
}

function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function MediaSelectorModal({
  open,
  onOpenChange,
  onSelect,
  selectionMode = "single",
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (path: string) => void;
  selectionMode?: "single" | "multi";
}) {
  const queryClient = useQueryClient();
  const [selectedPath, setSelectedPath] = useState("");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [history, setHistory] = useState<string[]>([""]);
  const [historyIndex, setHistoryIndex] = useState(0);

  const canGoBack = historyIndex > 0;
  const canGoForward = historyIndex < history.length - 1;

  const navigateTo = (path: string) => {
    const newHistory = history.slice(0, historyIndex + 1);
    newHistory.push(path);
    setHistory(newHistory);
    setHistoryIndex(newHistory.length - 1);
    setSelectedPath(path);
  };

  const goBack = () => {
    if (canGoBack) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      setSelectedPath(history[newIndex]);
    }
  };

  const goForward = () => {
    if (canGoForward) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      setSelectedPath(history[newIndex]);
    }
  };

  const mediaQuery = useQuery({
    queryKey: ["mediaItems", selectedPath],
    queryFn: () => fetchMediaItems(selectedPath),
    enabled: open,
  });

  useEffect(() => {
    if (!open) {
      setSelectedFile(null);
      setSelectedFiles(new Set());
    }
  }, [open]);

  const uploadMutation = useMutation({
    mutationFn: async (files: FileList) => {
      for (const file of Array.from(files)) {
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
          folder: selectedPath,
        });
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["mediaItems", selectedPath] });
    },
  });

  const handleUpload = (files: FileList) => {
    uploadMutation.mutate(files);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleUpload(e.dataTransfer.files);
    }
  };

  const handleFileSelect = (publicUrl: string) => {
    if (selectionMode === "single") {
      setSelectedFile(selectedFile === publicUrl ? null : publicUrl);
    } else {
      const newSelection = new Set(selectedFiles);
      if (newSelection.has(publicUrl)) {
        newSelection.delete(publicUrl);
      } else {
        newSelection.add(publicUrl);
      }
      setSelectedFiles(newSelection);
    }
  };

  const handleConfirm = () => {
    if (selectionMode === "single" && selectedFile) {
      onSelect(selectedFile);
      onOpenChange(false);
    } else if (selectionMode === "multi" && selectedFiles.size > 0) {
      onSelect(Array.from(selectedFiles).join(","));
      onOpenChange(false);
    }
  };

  const handleFolderClick = (folderPath: string) => {
    navigateTo(folderPath);
  };

  const items = mediaQuery.data || [];
  const folders = items.filter((item) => item.type === "dir");
  const files = items
    .filter((item) => item.type === "file")
    .map((item) => ({
      ...item,
      relativePath: getRelativePath(item.path),
    }));

  const filteredFiles = files.filter((item) => {
    if (!searchQuery) return true;
    return item.name.toLowerCase().includes(searchQuery.toLowerCase());
  });

  const filteredFolders = folders.filter((item) => {
    if (!searchQuery) return true;
    return item.name.toLowerCase().includes(searchQuery.toLowerCase());
  });

  const selectionCount =
    selectionMode === "single" ? (selectedFile ? 1 : 0) : selectedFiles.size;

  const breadcrumbs = selectedPath ? selectedPath.split("/") : [];

  if (!open) return null;

  const modalContent = (
    <>
      <div
        className="animate-in fade-in-0 fixed inset-0 z-50 bg-black/80"
        onClick={() => onOpenChange(false)}
      />
      <div className="animate-in fade-in-0 zoom-in-95 fixed top-[50%] left-[50%] z-50 flex h-[80vh] w-full max-w-4xl translate-x-[-50%] translate-y-[-50%] flex-col overflow-hidden rounded-sm border bg-white shadow-lg">
        {/* Header with search and breadcrumbs */}
        <div className="shrink-0">
          <div className="flex items-center gap-1.5 border-b border-neutral-200 px-4 py-3">
            <Icon
              icon="mdi:magnify"
              className="shrink-0 text-lg text-neutral-400"
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search files..."
              className="flex-1 bg-transparent text-sm placeholder:text-neutral-400 focus:outline-none"
            />
          </div>
          <div className="flex items-center gap-2 border-b border-neutral-200 py-2 pr-4 pl-2 text-sm text-neutral-600">
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={goBack}
                disabled={!canGoBack}
                className={cn([
                  "cursor-pointer rounded p-1 hover:bg-neutral-100",
                  !canGoBack && "cursor-not-allowed opacity-30",
                ])}
              >
                <ChevronLeftIcon className="size-4" />
              </button>
              <button
                type="button"
                onClick={goForward}
                disabled={!canGoForward}
                className={cn([
                  "cursor-pointer rounded p-1 hover:bg-neutral-100",
                  !canGoForward && "cursor-not-allowed opacity-30",
                ])}
              >
                <ChevronRightIcon className="size-4" />
              </button>
            </div>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => navigateTo("")}
                className={cn([
                  "cursor-pointer hover:text-neutral-900",
                  selectedPath === "" && "font-medium text-neutral-900",
                ])}
              >
                images
              </button>
              {breadcrumbs.map((crumb, i) => (
                <span key={i} className="flex items-center gap-1">
                  <span className="text-neutral-400">/</span>
                  <button
                    type="button"
                    onClick={() =>
                      navigateTo(breadcrumbs.slice(0, i + 1).join("/"))
                    }
                    className={cn([
                      "cursor-pointer hover:text-neutral-900",
                      i === breadcrumbs.length - 1 &&
                        "font-medium text-neutral-900",
                    ])}
                  >
                    {crumb}
                  </button>
                </span>
              ))}
            </div>
          </div>
        </div>

        {/* Body - Files and Folders */}
        <div
          className={cn([
            "flex-1 overflow-y-auto p-4",
            dragOver && "bg-blue-50",
          ])}
          onDrop={handleDrop}
          onDragOver={(e) => {
            e.preventDefault();
            setDragOver(true);
          }}
          onDragLeave={() => setDragOver(false)}
        >
          {mediaQuery.isLoading ? (
            <div className="flex h-full items-center justify-center text-neutral-500">
              <Icon icon="mdi:loading" className="mr-2 animate-spin text-2xl" />
              Loading...
            </div>
          ) : filteredFolders.length === 0 && filteredFiles.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center text-neutral-500">
              <Icon icon="mdi:folder-open-outline" className="mb-3 text-4xl" />
              <p className="text-sm">No files found</p>
              <p className="mt-1 text-xs">
                Drag and drop files here or click "Add new file"
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-4">
              {filteredFolders.length > 0 && (
                <div>
                  <p className="mb-2 text-xs font-medium tracking-wide text-neutral-500 uppercase">
                    Folders
                  </p>
                  <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6">
                    {filteredFolders.map((folder) => (
                      <div
                        key={folder.path}
                        className="group relative cursor-pointer overflow-hidden rounded border border-neutral-200 transition-all hover:border-neutral-300"
                        onClick={() =>
                          handleFolderClick(getRelativePath(folder.path))
                        }
                      >
                        <div className="flex aspect-square items-center justify-center bg-neutral-50">
                          <Icon
                            icon="mdi:folder"
                            className="text-4xl text-amber-400"
                          />
                        </div>
                        <div className="p-1.5">
                          <p
                            className="truncate text-xs text-neutral-700"
                            title={folder.name}
                          >
                            {folder.name}
                          </p>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {filteredFiles.length > 0 && (
                <div>
                  <p className="mb-2 text-xs font-medium tracking-wide text-neutral-500 uppercase">
                    Files
                  </p>
                  <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6">
                    {filteredFiles.map((item) => {
                      const isSelected =
                        selectionMode === "single"
                          ? selectedFile === item.publicUrl
                          : selectedFiles.has(item.publicUrl);
                      return (
                        <div
                          key={item.path}
                          className={cn([
                            "group relative cursor-pointer overflow-hidden rounded border transition-all",
                            isSelected
                              ? "border-blue-500 ring-1 ring-blue-500"
                              : "border-neutral-200 hover:border-neutral-300",
                          ])}
                          onClick={() => handleFileSelect(item.publicUrl)}
                        >
                          <div className="flex aspect-square items-center justify-center overflow-hidden bg-neutral-100">
                            {item.publicUrl ? (
                              <img
                                src={item.publicUrl}
                                alt={item.name}
                                className="h-full w-full object-cover"
                                loading="lazy"
                              />
                            ) : (
                              <Icon
                                icon="mdi:file-outline"
                                className="text-3xl text-neutral-400"
                              />
                            )}
                          </div>
                          <div className="p-1.5">
                            <p
                              className="truncate text-xs text-neutral-700"
                              title={item.name}
                            >
                              {item.name}
                            </p>
                            <p className="text-xs text-neutral-400">
                              {formatFileSize(item.size)}
                            </p>
                          </div>
                          {isSelected && (
                            <div className="absolute top-1 left-1">
                              <div className="flex h-4 w-4 items-center justify-center rounded-full bg-blue-500">
                                <Icon
                                  icon="mdi:check"
                                  className="text-xs text-white"
                                />
                              </div>
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex h-14 shrink-0 items-center justify-between border-t border-neutral-200 px-4">
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            disabled={uploadMutation.isPending}
            className="cursor-pointer rounded-xs border border-neutral-200 px-2 py-1.5 font-mono text-xs font-medium text-neutral-700 hover:bg-neutral-50 disabled:opacity-50"
          >
            {uploadMutation.isPending ? "Uploading..." : "Add new file"}
          </button>
          <div className="flex items-center gap-3">
            {selectionCount > 0 && (
              <span className="font-mono text-xs text-neutral-500">
                {selectionCount} selected
              </span>
            )}
            <button
              type="button"
              onClick={handleConfirm}
              disabled={selectionCount === 0}
              className={cn([
                "cursor-pointer rounded-xs px-2 py-1.5 font-mono text-xs font-medium",
                selectionCount > 0
                  ? "bg-neutral-900 text-white hover:bg-neutral-800"
                  : "cursor-not-allowed bg-neutral-100 text-neutral-400",
              ])}
            >
              Confirm
            </button>
          </div>
        </div>
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept="image/*,video/*"
          className="hidden"
          onChange={(e) => e.target.files && handleUpload(e.target.files)}
        />
      </div>
    </>
  );

  return createPortal(modalContent, document.body);
}
