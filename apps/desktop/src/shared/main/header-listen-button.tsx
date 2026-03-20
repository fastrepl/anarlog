import { ChevronDown, MicOff } from "lucide-react";
import {
  type MouseEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
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

function useIsNarrow(threshold = 720) {
  const [isNarrow, setIsNarrow] = useState(() => window.innerWidth < threshold);

  useEffect(() => {
    const handleResize = () => {
      setIsNarrow(window.innerWidth < threshold);
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [threshold]);

  return isNarrow;
}

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

  if (isRecording) return true;
  if (loading) return false;
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
  const { status, stop, amplitude, muted } = useListener((state) => ({
    status: state.live.status,
    stop: state.stop,
    amplitude: state.live.amplitude,
    muted: state.live.muted,
  }));
  const [open, setOpen] = useState(false);
  const isNarrow = useIsNarrow();
  const isActive = status === "active";
  const isFinalizing = status === "finalizing";
  const isRecording = isActive || isFinalizing;
  const effectivelyDisabled = isFinalizing || (!isRecording && isDisabled);

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

  const handleButtonClick = isActive ? stop : handleClick;

  const mainButton = (
    <button
      type="button"
      onClick={handleButtonClick}
      onMouseDown={isRecording ? undefined : handleMenuMouseDown}
      onContextMenu={isRecording ? undefined : handleOpenMenu}
      disabled={effectivelyDisabled}
      className={cn([
        "flex h-full items-center justify-center",
        isRecording ? "px-3" : "pl-3",
      ])}
      aria-label={
        isFinalizing
          ? "Finalizing"
          : isActive
            ? "Stop listening"
            : "New meeting"
      }
    >
      {isRecording ? (
        <div
          className={cn([
            "items-center justify-items-center",
            isFinalizing
              ? "flex gap-2"
              : "grid [&>*]:col-start-1 [&>*]:row-start-1",
          ])}
        >
          {isFinalizing ? (
            <>
              <span className="size-2 animate-pulse rounded-full bg-yellow-400" />
              {!isNarrow && (
                <span className="whitespace-nowrap">Finalizing</span>
              )}
            </>
          ) : (
            <>
              <span className="flex items-center gap-2 transition-opacity duration-150 group-hover:opacity-0">
                {muted && <MicOff className="size-3.5 text-red-500" />}
                <DancingSticks
                  amplitude={Math.min(
                    Math.hypot(amplitude.mic, amplitude.speaker),
                    1,
                  )}
                  color="#dc2626"
                  height={20}
                  width={isNarrow ? 24 : 72}
                  stickWidth={3}
                  gap={2}
                />
              </span>
              <span className="inline-flex items-center gap-2 whitespace-nowrap opacity-0 transition-opacity duration-150 group-hover:opacity-100">
                <span className="size-2.5 rounded-xs bg-red-600" />
                {!isNarrow && <span>Stop</span>}
              </span>
            </>
          )}
        </div>
      ) : (
        <span className="inline-flex shrink-0 items-center gap-2">
          <RecordingIcon />
          {!isNarrow && <span className="whitespace-nowrap">New meeting</span>}
        </span>
      )}
    </button>
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <div
          ref={containerRef}
          className={cn([
            "group inline-flex h-8 items-center rounded-full text-sm font-medium select-none",
            "border-1",
            isRecording
              ? "border-red-400 bg-red-50 text-red-600"
              : "border-stone-600 bg-stone-800 text-white",
            "transition-all duration-200 ease-out",
            !isFinalizing &&
              (isRecording
                ? "hover:bg-red-50 hover:text-red-700"
                : "hover:bg-stone-700"),
            isFinalizing && "cursor-wait",
            effectivelyDisabled && "opacity-50",
          ])}
          onMouseDownCapture={handleMenuMouseDown}
          onContextMenu={handleOpenMenu}
        >
          {warningMessage && !isRecording ? (
            <Tooltip delayDuration={0}>
              <TooltipTrigger asChild>
                <span className="inline-flex h-full">{mainButton}</span>
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
            mainButton
          )}
          {!isRecording && (
            <PopoverTrigger asChild>
              <button
                type="button"
                className="flex h-full w-7 shrink-0 cursor-pointer items-center justify-center rounded-r-full text-white/70 transition-colors select-none hover:text-white"
                onMouseDown={handleMenuMouseDown}
                onClick={(event) => {
                  event.stopPropagation();
                }}
              >
                <ChevronDown className="size-3" />
                <span className="sr-only">More options</span>
              </button>
            </PopoverTrigger>
          )}
        </div>
      </PopoverAnchor>
      {!isRecording && (
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
      )}
    </Popover>
  );
}
