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

import type { FileHandlerConfig } from "@hypr/editor/note";

import { useFileUpload } from "~/shared/hooks/useFileUpload";
import { isAudioUploadFile, useUploadFile } from "~/stt/useUploadFile";

export function useNoteFileHandlerConfig(sessionId: string) {
  const onFileUpload = useFileUpload(sessionId);
  const { processAudioFile } = useUploadFile(sessionId);
  const [isAudioDragActive, setIsAudioDragActive] = useState(false);
  const audioDragDepthRef = useRef(0);

  const processAudioFiles = useCallback(
    (files: File[]) => {
      const audioFiles = files.filter(isAudioUploadFile);
      if (audioFiles.length === 0) {
        return false;
      }

      audioFiles.forEach((file) => processAudioFile(file));
      return true;
    },
    [processAudioFile],
  );

  const handleDrop = useCallback(
    (files: File[]) => (processAudioFiles(files) ? true : undefined),
    [processAudioFiles],
  );

  const resetAudioDrag = useCallback(() => {
    audioDragDepthRef.current = 0;
    setIsAudioDragActive(false);
  }, []);

  const prepareAudioDragEvent = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "copy";
      setIsAudioDragActive(true);
    },
    [],
  );

  const handleDragEnterCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      if (!hasAudioUploadDrag(event.dataTransfer)) {
        return;
      }

      if (audioDragDepthRef.current === 0) {
        focusCurrentWindowForAudioDrop();
      }

      audioDragDepthRef.current += 1;
      prepareAudioDragEvent(event);
    },
    [prepareAudioDragEvent],
  );

  const handleDragOverCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      if (
        audioDragDepthRef.current === 0 &&
        !hasAudioUploadDrag(event.dataTransfer)
      ) {
        return;
      }

      prepareAudioDragEvent(event);
    },
    [prepareAudioDragEvent],
  );

  const handleDragLeaveCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      if (
        audioDragDepthRef.current === 0 &&
        !hasAudioUploadDrag(event.dataTransfer)
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      audioDragDepthRef.current = Math.max(0, audioDragDepthRef.current - 1);
      if (audioDragDepthRef.current === 0) {
        setIsAudioDragActive(false);
      }
    },
    [],
  );

  const handleDropCapture = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      const files = Array.from(event.dataTransfer.files ?? []);
      if (!processAudioFiles(files)) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      resetAudioDrag();
    },
    [processAudioFiles, resetAudioDrag],
  );

  const fileHandlerConfig = useMemo<FileHandlerConfig>(
    () => ({ onFileUpload, onDrop: handleDrop }),
    [handleDrop, onFileUpload],
  );

  const audioDropTargetProps = useMemo<HTMLAttributes<HTMLDivElement>>(
    () => ({
      onDragEnterCapture: handleDragEnterCapture,
      onDragOverCapture: handleDragOverCapture,
      onDragLeaveCapture: handleDragLeaveCapture,
      onDropCapture: handleDropCapture,
      onDragEndCapture: resetAudioDrag,
    }),
    [
      handleDragEnterCapture,
      handleDragLeaveCapture,
      handleDragOverCapture,
      handleDropCapture,
      resetAudioDrag,
    ],
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

function hasAudioUploadDrag(dataTransfer: DataTransfer) {
  const items = Array.from(dataTransfer.items ?? []);
  if (items.length > 0) {
    return items.some((item) => {
      if (item.kind !== "file") {
        return false;
      }

      if (item.type.startsWith("audio/")) {
        return true;
      }

      const file = item.getAsFile();
      return file ? isAudioUploadFile(file) : false;
    });
  }

  return Array.from(dataTransfer.files ?? []).some(isAudioUploadFile);
}

function focusCurrentWindowForAudioDrop() {
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
    console.error("Failed to focus window for audio drop", error);
  }
}
