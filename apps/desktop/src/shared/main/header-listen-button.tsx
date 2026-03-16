import { ChevronDown } from "lucide-react";
import {
  type MouseEvent,
  useCallback,
  useEffect,
  useMemo,
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

import { useCreateSession, useNewNoteAndListen } from "./useNewNote";

import { useNetwork } from "~/contexts/network";
import {
  useImportAudioToTranscript,
  useImportTranscriptToSession,
  selectImportFile,
} from "~/session/components/note-input/session-import";
import {
  ActionableTooltipContent,
  RecordingIcon,
  useHasTranscript,
} from "~/session/components/shared";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { useSTTConnection } from "~/stt/useSTTConnection";

type PendingHeaderAction = {
  type: "upload_audio" | "upload_transcript";
  sessionId: string;
  path: string;
};

export function HeaderListenButton() {
  const visible = useHeaderListenVisible();
  const [pendingAction, setPendingAction] =
    useState<PendingHeaderAction | null>(null);
  const clearPendingAction = useCallback(() => {
    setPendingAction(null);
  }, []);

  if (!visible && !pendingAction) {
    return null;
  }

  return (
    <>
      {visible && (
        <HeaderListenButtonInner
          isBusy={pendingAction !== null}
          onQueueAction={setPendingAction}
        />
      )}
      {pendingAction && (
        <PendingHeaderActionRunner
          action={pendingAction}
          onComplete={clearPendingAction}
        />
      )}
    </>
  );
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

function HeaderListenButtonInner({
  isBusy,
  onQueueAction,
}: {
  isBusy: boolean;
  onQueueAction: (action: PendingHeaderAction) => void;
}) {
  const { isDisabled, warningMessage } = useHeaderListenState();
  const [open, setOpen] = useState(false);
  const [isSelectingFile, setIsSelectingFile] = useState(false);
  const [menuWidth, setMenuWidth] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const handleClick = useNewNoteAndListen();
  const createSession = useCreateSession();
  const openNew = useTabs((state) => state.openNew);

  const disableUploadAudio = isDisabled || isBusy || isSelectingFile;
  const disableUploadTranscript = isBusy || isSelectingFile;
  const menuDisabled = useMemo(
    () => disableUploadAudio && disableUploadTranscript,
    [disableUploadAudio, disableUploadTranscript],
  );

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

  const createTranscriptSession = useCallback(
    () =>
      createSession({
        view: { type: "transcript" },
        autoStart: null,
      }),
    [createSession],
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

  const handleUploadAudio = useCallback(async () => {
    if (disableUploadAudio) {
      return;
    }

    setIsSelectingFile(true);

    try {
      const path = await selectImportFile("audio");
      if (!path) {
        return;
      }

      setOpen(false);
      onQueueAction({
        type: "upload_audio",
        sessionId: createTranscriptSession(),
        path,
      });
    } finally {
      setIsSelectingFile(false);
    }
  }, [createTranscriptSession, disableUploadAudio, onQueueAction]);

  const handleUploadTranscript = useCallback(async () => {
    if (disableUploadTranscript) {
      return;
    }

    setIsSelectingFile(true);

    try {
      const path = await selectImportFile("transcript");
      if (!path) {
        return;
      }

      setOpen(false);
      onQueueAction({
        type: "upload_transcript",
        sessionId: createTranscriptSession(),
        path,
      });
    } finally {
      setIsSelectingFile(false);
    }
  }, [createTranscriptSession, disableUploadTranscript, onQueueAction]);

  const button = (
    <button
      type="button"
      onClick={handleClick}
      onMouseDown={handleMenuMouseDown}
      onContextMenu={handleOpenMenu}
      disabled={isDisabled || isBusy || isSelectingFile}
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

  const moreButton = (
    <button
      type="button"
      className="absolute inset-y-0 right-0 z-10 inline-flex w-9 cursor-pointer items-center justify-center rounded-r-full bg-transparent text-white/70 transition-colors select-none hover:text-white disabled:cursor-default disabled:opacity-50"
      disabled={menuDisabled}
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
          <PopoverTrigger asChild>{moreButton}</PopoverTrigger>
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
            disabled={disableUploadAudio}
            onClick={() => {
              void handleUploadAudio();
            }}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 w-full justify-center rounded-[0.95rem] px-3 text-sm text-stone-900 shadow-none hover:bg-black/6 hover:text-stone-950 focus-visible:ring-0 focus-visible:outline-none"
            disabled={disableUploadTranscript}
            onClick={() => {
              void handleUploadTranscript();
            }}
          >
            <span className="text-sm">Upload transcript</span>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function PendingHeaderActionRunner({
  action,
  onComplete,
}: {
  action: PendingHeaderAction;
  onComplete: () => void;
}) {
  const hasRunRef = useRef(false);
  const { importAudio } = useImportAudioToTranscript(action.sessionId);
  const { importTranscript } = useImportTranscriptToSession(action.sessionId);

  useEffect(() => {
    if (hasRunRef.current) {
      return;
    }

    hasRunRef.current = true;

    let active = true;

    const run = async () => {
      try {
        if (action.type === "upload_audio") {
          await importAudio(action.path);
          return;
        }

        await importTranscript(action.path);
      } catch (error) {
        console.error("[header_meeting_action] failed:", error);
      } finally {
        if (active) {
          onComplete();
        }
      }
    };

    void run();

    return () => {
      active = false;
    };
  }, [action, importAudio, importTranscript, onComplete]);

  return null;
}
