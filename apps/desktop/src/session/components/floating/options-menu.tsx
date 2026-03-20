import { EllipsisVerticalIcon } from "lucide-react";
import { useCallback, useState } from "react";

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

import { useTabs } from "~/store/zustand/tabs";
import type { Tab } from "~/store/zustand/tabs/schema";
import { useStartListening } from "~/stt/useStartListening";
import { useUploadFile } from "~/stt/useUploadFile";

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
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
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

  const handleUploadAudio = useCallback(() => {
    if (disabled) {
      return;
    }
    setOpen(false);
    uploadAudio();
  }, [disabled, uploadAudio]);

  const handleUploadTranscript = useCallback(() => {
    if (disabled) {
      return;
    }
    setOpen(false);
    uploadTranscript();
  }, [disabled, uploadTranscript]);

  const handleStartBatchRecording = useCallback(() => {
    if (disabled) {
      return;
    }

    setOpen(false);

    if (sessionTab) {
      updateSessionTabState(sessionTab, {
        ...sessionTab.state,
        view: { type: "transcript" },
      });
    }

    startBatchRecording();
  }, [
    disabled,
    sessionTab,
    setOpen,
    startBatchRecording,
    updateSessionTabState,
  ]);

  const moreButton = (
    <button
      className="absolute top-1/2 right-2 z-10 -translate-y-1/2 cursor-pointer text-white/70 transition-colors hover:text-white disabled:opacity-50"
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        setOpen(true);
      }}
    >
      <EllipsisVerticalIcon className="size-4" />
      <span className="sr-only">More options</span>
    </button>
  );

  if (disabled && warningMessage) {
    return (
      <div className="relative flex items-center">
        {children}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span className="inline-block">{moreButton}</span>
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
      </div>
    );
  }

  if (disabled) {
    return (
      <div className="relative flex items-center">
        {children}
        {moreButton}
      </div>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div className="relative flex items-center">
          {children}
          {moreButton}
        </div>
      </PopoverTrigger>
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
            onClick={handleStartBatchRecording}
          >
            <span className="text-sm">Record only</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            onClick={handleUploadAudio}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            onClick={handleUploadTranscript}
          >
            <span className="text-sm">Upload transcript</span>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
