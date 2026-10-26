import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  type DragEvent,
  type HTMLAttributes,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react";

import type { FileHandlerConfig } from "@anlg/editor/note";
import { toast } from "@anlg/ui/components/ui/toast";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import {
  importMarkdownFilesAsNotes,
  isFileDrag,
  markdownDragOverlayForPaths,
  matchMarkdownDrag,
  matchMarkdownDrop,
} from "./markdown-import";

import { useFileUpload } from "~/shared/hooks/useFileUpload";
import { isAudioUploadFile, useUploadFile } from "~/stt/useUploadFile";

type DropTargetMatch = {
  active: boolean;
  title?: string;
  subtitle?: string;
};

type DropOutcome = "handled" | "reset" | "ignored";

export function useNoteFileHandlerConfig(sessionId: string) {
  const onFileUpload = useFileUpload(sessionId);
  const { processAudioFile } = useUploadFile(sessionId);

  const processAudioDrop = useCallback(
    (files: File[], items?: DataTransferItemList) => {
      const audioDrop = getAudioDrop(files, items);
      if (!audioDrop) {
        return null;
      }

      if (audioDrop.allowUnknownAudio) {
        processAudioFile(audioDrop.audioFile, {
          allowUnknownAudio: true,
          contentType: audioDrop.contentType,
        });
      } else {
        processAudioFile(audioDrop.audioFile);
      }
      return { remainingFiles: audioDrop.remainingFiles };
    },
    [processAudioFile],
  );

  const handleDrop = useCallback(
    (files: File[], _pos?: number, items?: DataTransferItemList) => {
      const result = processAudioDrop(files, items);
      if (!result) {
        return undefined;
      }

      return result.remainingFiles.length === 0 ? true : result;
    },
    [processAudioDrop],
  );

  const handlePaste = useCallback(
    (files: File[], items?: DataTransferItemList) =>
      handleDrop(files, undefined, items),
    [handleDrop],
  );
  const handleFileUploadError = useCallback((error: unknown) => {
    toast.error(
      error instanceof Error ? error.message : "Could not add this attachment.",
    );
  }, []);

  const dropAudioFiles = useCallback(
    (files: File[], items?: DataTransferItemList): DropOutcome => {
      if (files.length !== 1) {
        return "ignored";
      }

      const audioDrop = getAudioDrop(files, items);
      if (!audioDrop) {
        return "ignored";
      }

      if (audioDrop.remainingFiles.length > 0) {
        return "reset";
      }

      if (audioDrop.allowUnknownAudio) {
        processAudioFile(audioDrop.audioFile, {
          allowUnknownAudio: true,
          contentType: audioDrop.contentType,
        });
      } else {
        processAudioFile(audioDrop.audioFile);
      }
      return "handled";
    },
    [processAudioFile],
  );

  const { isDragActive: isAudioDragActive, targetProps: audioDropTargetProps } =
    useFileDropTarget({
      dropFiles: dropAudioFiles,
      matchDrag: matchAudioDrag,
    });

  const fileHandlerConfig = useMemo<FileHandlerConfig>(
    () => ({
      onFileUpload,
      onFileUploadError: handleFileUploadError,
      onDrop: handleDrop,
      onPaste: handlePaste,
    }),
    [handleDrop, handleFileUploadError, handlePaste, onFileUpload],
  );

  return useMemo(
    () => ({
      audioDropTargetProps,
      fileHandlerConfig,
      isAudioDragActive,
    }),
    [audioDropTargetProps, fileHandlerConfig, isAudioDragActive],
  );
}

export function useMarkdownDropTarget() {
  const [hoverPaths, setHoverPaths] = useState<string[] | null>(null);

  useMountEffect(() => {
    if (!isTauri()) {
      return;
    }
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter") {
          setHoverPaths(payload.paths);
        } else if (payload.type === "leave" || payload.type === "drop") {
          setHoverPaths(null);
        }
      })
      .then((stop) => {
        if (cancelled) {
          stop();
        } else {
          unlisten = stop;
        }
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  });

  const matchMarkdownHover = useCallback(
    (dataTransfer: DataTransfer): DropTargetMatch => {
      if (hoverPaths) {
        return markdownDragOverlayForPaths(hoverPaths);
      }
      if (!isFileDrag(dataTransfer)) {
        return { active: false };
      }
      return matchMarkdownDrag(dataTransfer);
    },
    [hoverPaths],
  );

  const dropMarkdownFiles = useCallback((files: File[]): DropOutcome => {
    const matched = matchMarkdownDrop(files);
    if (!matched) {
      return "ignored";
    }

    void importMarkdownFilesAsNotes(matched);
    return "handled";
  }, []);

  const {
    dragSubtitle,
    dragTitle,
    isDragActive: isMarkdownDragActive,
    targetProps: markdownDropTargetProps,
  } = useFileDropTarget({
    dropFiles: dropMarkdownFiles,
    matchDrag: matchMarkdownHover,
  });

  return useMemo(
    () => ({
      isMarkdownDragActive,
      markdownDragSubtitle: dragSubtitle ?? "",
      markdownDragTitle: dragTitle ?? "",
      markdownDropTargetProps,
    }),
    [dragSubtitle, dragTitle, isMarkdownDragActive, markdownDropTargetProps],
  );
}

function useFileDropTarget({
  matchDrag,
  dropFiles,
}: {
  matchDrag: (dataTransfer: DataTransfer) => DropTargetMatch;
  dropFiles: (files: File[], items?: DataTransferItemList) => DropOutcome;
}) {
  const [isDragActive, setIsDragActive] = useState(false);
  const [dragTitle, setDragTitle] = useState<string | null>(null);
  const [dragSubtitle, setDragSubtitle] = useState<string | null>(null);
  const dragDepthRef = useRef(0);

  const resetDrag = useCallback(() => {
    dragDepthRef.current = 0;
    setIsDragActive(false);
    setDragTitle(null);
    setDragSubtitle(null);
  }, []);

  const prepareDragEvent = useCallback((event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = "copy";
  }, []);

  const showDragMatch = useCallback((match: DropTargetMatch) => {
    setDragTitle(match.title ?? null);
    setDragSubtitle(match.subtitle ?? null);
    setIsDragActive(true);
  }, []);

  const handleDragEnterCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      const match = matchDrag(event.dataTransfer);
      if (!match.active) {
        return;
      }

      if (dragDepthRef.current === 0) {
        focusCurrentWindowForFileDrop();
      }

      dragDepthRef.current += 1;
      prepareDragEvent(event);
      showDragMatch(match);
    },
    [matchDrag, prepareDragEvent, showDragMatch],
  );

  const handleDragOverCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      if (dragDepthRef.current === 0) {
        const match = matchDrag(event.dataTransfer);
        if (!match.active) {
          return;
        }
        showDragMatch(match);
      } else {
        setIsDragActive(true);
      }

      prepareDragEvent(event);
    },
    [matchDrag, prepareDragEvent, showDragMatch],
  );

  const handleDragLeaveCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      if (dragDepthRef.current === 0 && !matchDrag(event.dataTransfer).active) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
      if (dragDepthRef.current === 0) {
        setIsDragActive(false);
        setDragTitle(null);
        setDragSubtitle(null);
      }
    },
    [matchDrag],
  );

  const handleDropCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      const outcome = dropFiles(
        Array.from(event.dataTransfer.files ?? []),
        event.dataTransfer.items,
      );
      if (outcome === "ignored") {
        return;
      }

      if (outcome === "handled") {
        event.preventDefault();
        event.stopPropagation();
      }
      resetDrag();
    },
    [dropFiles, resetDrag],
  );

  const targetProps = useMemo<HTMLAttributes<HTMLDivElement>>(
    () => ({
      onDragEnterCapture: handleDragEnterCapture,
      onDragOverCapture: handleDragOverCapture,
      onDragLeaveCapture: handleDragLeaveCapture,
      onDropCapture: handleDropCapture,
      onDragEndCapture: resetDrag,
    }),
    [
      handleDragEnterCapture,
      handleDragLeaveCapture,
      handleDragOverCapture,
      handleDropCapture,
      resetDrag,
    ],
  );

  return useMemo(
    () => ({
      dragSubtitle,
      dragTitle,
      isDragActive,
      targetProps,
    }),
    [dragSubtitle, dragTitle, isDragActive, targetProps],
  );
}

function matchAudioDrag(dataTransfer: DataTransfer): DropTargetMatch {
  return { active: hasSingleAudioUploadDrag(dataTransfer) };
}

function hasSingleAudioUploadDrag(dataTransfer: DataTransfer) {
  const items = Array.from(dataTransfer.items ?? []);
  if (items.length > 0) {
    if (items.length !== 1) {
      return false;
    }

    const [item] = items;
    if (item.kind !== "file") {
      return false;
    }

    if (item.type.startsWith("audio/")) {
      return true;
    }

    const file = item.getAsFile();
    return file ? isAudioUploadFile(file) : false;
  }

  const files = Array.from(dataTransfer.files ?? []);
  return files.length === 1 && isAudioUploadFile(files[0]);
}

function getAudioDrop(files: File[], items?: DataTransferItemList) {
  const dataTransferItems = Array.from(items ?? []).filter(
    (item) => item.kind === "file",
  );
  const audioFileIndex = files.findIndex((file, index) =>
    isAudioDropFile(file, dataTransferItems[index]),
  );
  if (audioFileIndex === -1) {
    return null;
  }
  const audioFile = files[audioFileIndex];

  return {
    allowUnknownAudio: !isAudioUploadFile(audioFile),
    audioFile,
    contentType:
      audioFile.type || dataTransferItems[audioFileIndex]?.type || undefined,
    remainingFiles: files.filter((file) => file !== audioFile),
  };
}

function isAudioDropFile(file: File, item?: DataTransferItem) {
  return isAudioUploadFile(file) || item?.type.startsWith("audio/") === true;
}

function focusCurrentWindowForFileDrop() {
  if (!isTauri()) {
    return;
  }

  void bringCurrentWindowToFront();
}

async function bringCurrentWindowToFront() {
  try {
    const currentWindow = getCurrentWindow();
    await currentWindow.show();
    await currentWindow.unminimize();
    await currentWindow.setFocus();
  } catch (error) {
    console.error("Failed to focus window for file drop", error);
  }
}
