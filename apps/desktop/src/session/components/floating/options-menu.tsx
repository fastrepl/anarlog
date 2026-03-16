import { EllipsisVerticalIcon } from "lucide-react";
import { type MouseEvent, useCallback, useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { ActionableTooltipContent } from "./shared";

import { useSessionImportPickerActions } from "~/session/components/note-input/session-import";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useStartListening } from "~/stt/useStartListening";

export function OptionsMenu({
  sessionId,
  disabled,
  warningMessage,
  onConfigure,
  children,
}: {
  sessionId: string;
  disabled: boolean;
  warningMessage: string;
  onConfigure?: () => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const { selectAndImportAudio, selectAndImportTranscript } =
    useSessionImportPickerActions(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const startBatchRecording = useStartListening(sessionId, {
    transcriptionMode: "batch",
  });
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const sessionTab = useTabs((state) => {
    const found = state.tabs.find(
      (tab): tab is Extract<Tab, { type: "sessions" }> =>
        tab.type === "sessions" && tab.id === sessionId,
    );
    return found ?? null;
  });

  const disableRecordOnly = disabled;
  const disableUploadAudio = disabled;
  const disableUploadTranscript = sessionMode === "running_batch";
  const menuDisabled = useMemo(
    () => disableRecordOnly && disableUploadAudio && disableUploadTranscript,
    [disableRecordOnly, disableUploadAudio, disableUploadTranscript],
  );

  const handleMenuMouseDown = useCallback((event: MouseEvent) => {
    if (event.button === 2) {
      event.preventDefault();
    }
  }, []);

  const handleOpenMenu = useCallback(
    (event: MouseEvent) => {
      if (menuDisabled) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setOpen(true);
    },
    [menuDisabled],
  );

  const handleUploadAudio = useCallback(() => {
    if (disableUploadAudio) {
      return;
    }

    void selectAndImportAudio().catch((error: unknown) => {
      console.error("[session_audio_import] failed:", error);
    });
  }, [disableUploadAudio, selectAndImportAudio]);

  const handleUploadTranscript = useCallback(() => {
    if (disableUploadTranscript) {
      return;
    }

    void selectAndImportTranscript().catch((error: unknown) => {
      console.error("[session_transcript_import] failed:", error);
    });
  }, [disableUploadTranscript, selectAndImportTranscript]);

  const handleStartBatchRecording = useCallback(() => {
    if (disableRecordOnly) {
      return;
    }

    setOpen(false);

    if (sessionTab) {
      updateSessionTabState(sessionTab, {
        ...sessionTab.state,
        view: { type: "transcript" },
      });
    }

    void startBatchRecording();
  }, [
    disableRecordOnly,
    sessionTab,
    startBatchRecording,
    updateSessionTabState,
  ]);

  const moreButton = (
    <button
      type="button"
      className="absolute top-1/2 right-2 z-10 -translate-y-1/2 cursor-pointer text-white/70 transition-colors hover:text-white disabled:cursor-default disabled:opacity-50"
      disabled={menuDisabled}
      onClick={(event) => {
        event.stopPropagation();
      }}
    >
      <EllipsisVerticalIcon className="size-4" />
      <span className="sr-only">More options</span>
    </button>
  );

  if (menuDisabled) {
    return (
      <div
        className="relative flex items-center"
        onMouseDownCapture={handleMenuMouseDown}
        onContextMenu={handleOpenMenu}
      >
        {children}
        {warningMessage ? (
          <Tooltip delayDuration={0}>
            <TooltipTrigger asChild>
              <span className="inline-flex">{moreButton}</span>
            </TooltipTrigger>
            <TooltipContent side="top" align="end">
              <ActionableTooltipContent
                message={warningMessage}
                action={
                  onConfigure
                    ? {
                        label: "Configure",
                        handleClick: onConfigure,
                      }
                    : undefined
                }
              />
            </TooltipContent>
          </Tooltip>
        ) : (
          moreButton
        )}
      </div>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <div
        className="relative flex items-center"
        onMouseDownCapture={handleMenuMouseDown}
        onContextMenu={handleOpenMenu}
      >
        {children}
        <PopoverTrigger asChild>{moreButton}</PopoverTrigger>
      </div>
      <PopoverContent
        side="top"
        align="center"
        sideOffset={8}
        className="w-43 rounded-xl p-1.5"
      >
        <div className="flex flex-col gap-1">
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            disabled={disableRecordOnly}
            onClick={handleStartBatchRecording}
          >
            <span className="text-sm">Record only</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            disabled={disableUploadAudio}
            onClick={handleUploadAudio}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            disabled={disableUploadTranscript}
            onClick={handleUploadTranscript}
          >
            <span className="text-sm">Upload transcript</span>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
