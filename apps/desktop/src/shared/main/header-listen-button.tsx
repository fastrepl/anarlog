import { ChevronDown } from "lucide-react";
import {
  type MouseEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useNewNoteAndListen, useNewNoteAndUpload } from "./useNewNote";

import { useNetwork } from "~/contexts/network";
import {
  ActionableTooltipContent,
  RecordingIcon,
  useHasTranscript,
} from "~/session/components/shared";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useSTTConnection } from "~/stt/useSTTConnection";

export function HeaderListenButton() {
  const visible = useHeaderListenVisible();

  if (!visible) {
    return null;
  }

  return <HeaderListenButtonInner />;
}

function useHeaderListenVisible() {
  const currentTab = useTabs((state) => state.currentTab);
  const liveStatus = useListener((state) => state.live.status);
  const loading = useListener((state) => state.live.loading);

  const sessionId = currentTab?.type === "sessions" ? currentTab.id : "";
  const hasTranscript = useHasTranscript(sessionId);

  const isRecording = liveStatus === "active" || liveStatus === "finalizing";

  if (isRecording || loading) return false;
  if (currentTab?.type === "empty") return true;
  if (currentTab?.type === "sessions" && hasTranscript) return true;

  return false;
}

function useHeaderListenState() {
  const { conn: sttConnection, local, isLocalModel } = useSTTConnection();
  const { isOnline } = useNetwork();

  const localServerStatus = local.data?.status ?? "unavailable";
  const isLocalServerLoading = localServerStatus === "loading";
  const isLocalModelNotDownloaded = localServerStatus === "not_downloaded";
  const isOfflineWithCloudModel = !isOnline && !isLocalModel;

  const isDisabled =
    !sttConnection ||
    isLocalServerLoading ||
    isLocalModelNotDownloaded ||
    isOfflineWithCloudModel;

  let warningMessage = "";
  if (isLocalModelNotDownloaded) {
    warningMessage = "Selected model is not downloaded.";
  } else if (isLocalServerLoading) {
    warningMessage = "Local STT server is starting up...";
  } else if (isOfflineWithCloudModel) {
    warningMessage = "You're offline. Use on-device models to continue.";
  } else if (!sttConnection) {
    warningMessage = "Transcription model not available.";
  }

  return { isDisabled, warningMessage };
}

function HeaderListenButtonInner() {
  const { isDisabled, warningMessage } = useHeaderListenState();
  const [menuWidth, setMenuWidth] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const handleClick = useNewNoteAndListen();
  const handleUpload = useNewNoteAndUpload();
  const openNew = useTabs((state) => state.openNew);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const node = containerRef.current;

    if (!node) {
      return;
    }

    const updateWidth = () => {
      setMenuWidth(node.offsetWidth);
    };

    updateWidth();

    const observer = new ResizeObserver(updateWidth);
    observer.observe(node);

    return () => {
      observer.disconnect();
    };
  }, []);

  const handleConfigure = useCallback(() => {
    openNew({ type: "ai", state: { tab: "transcription" } });
  }, [openNew]);

  const handleMenuMouseDown = useCallback((event: MouseEvent) => {
    if (event.button === 2) {
      event.preventDefault();
    }
  }, []);

  const handleOpenMenu = useCallback((event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setOpen(true);
  }, []);

  const handleUploadAudio = useCallback(() => {
    setOpen(false);
    handleUpload("audio").catch((error) => {
      console.error("[upload] audio dialog failed:", error);
    });
  }, [handleUpload]);

  const handleUploadTranscript = useCallback(() => {
    setOpen(false);
    handleUpload("transcript").catch((error) => {
      console.error("[upload] transcript dialog failed:", error);
    });
  }, [handleUpload]);

  const button = (
    <button
      type="button"
      onClick={handleClick}
      onMouseDown={handleMenuMouseDown}
      onContextMenu={handleOpenMenu}
      disabled={isDisabled}
      className={cn([
        "inline-flex items-center justify-center rounded-full text-sm font-medium text-white select-none",
        "gap-2",
        "h-8 pr-8 pl-4",
        "border-2 border-stone-600 bg-stone-800",
        "transition-all duration-200 ease-out",
        "hover:bg-stone-700",
        "disabled:opacity-50",
      ])}
    >
      <RecordingIcon />
      <span className="whitespace-nowrap">New meeting</span>
    </button>
  );

  const chevron = (
    <button
      type="button"
      className="absolute inset-y-0 right-0 z-10 inline-flex w-9 cursor-pointer items-center justify-center rounded-r-full bg-transparent text-white/70 transition-colors select-none hover:text-white"
      onMouseDown={handleMenuMouseDown}
      onClick={(event) => {
        event.stopPropagation();
      }}
    >
      <ChevronDown className="size-3.5" />
      <span className="sr-only">More options</span>
    </button>
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <div
          ref={containerRef}
          className="relative flex items-center select-none"
          onMouseDownCapture={handleMenuMouseDown}
          onContextMenu={handleOpenMenu}
        >
          {warningMessage ? (
            <Tooltip delayDuration={0}>
              <TooltipTrigger asChild>
                <span className="inline-flex">{button}</span>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                <ActionableTooltipContent
                  message={warningMessage}
                  action={{
                    label: "Configure",
                    handleClick: handleConfigure,
                  }}
                />
              </TooltipContent>
            </Tooltip>
          ) : (
            button
          )}
          <PopoverTrigger asChild>{chevron}</PopoverTrigger>
        </div>
      </PopoverAnchor>
      <PopoverContent
        side="bottom"
        align="end"
        sideOffset={4}
        style={menuWidth ? { width: menuWidth } : undefined}
        className={cn([
          "overflow-hidden rounded-[1.25rem] border border-white/70 p-1.5 ring-1 ring-black/6 outline-none",
          "bg-white/68 text-stone-900 shadow-[inset_0_1px_0_rgba(255,255,255,0.7),0_24px_48px_-24px_rgba(48,44,40,0.52),0_8px_18px_rgba(255,255,255,0.28)] backdrop-blur-md backdrop-saturate-150",
        ])}
      >
        <div className="flex flex-col gap-1">
          <Button
            variant="ghost"
            className="h-9 w-full justify-center rounded-[0.95rem] px-3 text-sm text-stone-900 shadow-none hover:bg-black/6 hover:text-stone-950 focus-visible:ring-0 focus-visible:outline-none"
            onClick={handleUploadAudio}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 w-full justify-center rounded-[0.95rem] px-3 text-sm text-stone-900 shadow-none hover:bg-black/6 hover:text-stone-950 focus-visible:ring-0 focus-visible:outline-none"
            onClick={handleUploadTranscript}
          >
            <span className="text-sm">Upload transcript</span>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
