import { EllipsisVerticalIcon } from "lucide-react";
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
import { useStartListening } from "~/stt/useStartListening";
import { useSTTConnection } from "~/stt/useSTTConnection";

type PendingHeaderAction =
  | {
      type: "record_only";
      sessionId: string;
    }
  | {
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
  const handleClick = useNewNoteAndListen();
  const createSession = useCreateSession();
  const openNew = useTabs((state) => state.openNew);

  const disableRecordOnly = isDisabled || isBusy || isSelectingFile;
  const disableUploadAudio = isDisabled || isBusy || isSelectingFile;
  const disableUploadTranscript = isBusy || isSelectingFile;
  const menuDisabled = useMemo(
    () => disableRecordOnly && disableUploadAudio && disableUploadTranscript,
    [disableRecordOnly, disableUploadAudio, disableUploadTranscript],
  );

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

  const handleRecordOnly = useCallback(() => {
    if (disableRecordOnly) {
      return;
    }

    setOpen(false);
    onQueueAction({
      type: "record_only",
      sessionId: createTranscriptSession(),
    });
  }, [createTranscriptSession, disableRecordOnly, onQueueAction]);

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
        "h-8 pr-9 pl-4",
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
      className="absolute top-1/2 right-2 z-10 -translate-y-1/2 cursor-pointer text-white/70 transition-colors select-none hover:text-white disabled:cursor-default disabled:opacity-50"
      disabled={menuDisabled}
      onMouseDown={handleMenuMouseDown}
      onClick={(event) => {
        event.stopPropagation();
      }}
    >
      <EllipsisVerticalIcon className="size-4" />
      <span className="sr-only">More options</span>
    </button>
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <div
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
      <PopoverContent
        side="bottom"
        align="end"
        sideOffset={8}
        className="w-43 rounded-xl p-1.5"
      >
        <div className="flex flex-col gap-1">
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            disabled={disableRecordOnly}
            onClick={handleRecordOnly}
          >
            <span className="text-sm">Record only</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
            disabled={disableUploadAudio}
            onClick={() => {
              void handleUploadAudio();
            }}
          >
            <span className="text-sm">Upload audio</span>
          </Button>
          <Button
            variant="ghost"
            className="h-9 justify-center px-3 whitespace-nowrap"
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
  const startBatchRecording = useStartListening(action.sessionId, {
    transcriptionMode: "batch",
  });
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
        if (action.type === "record_only") {
          await startBatchRecording();
          return;
        }

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
  }, [action, importAudio, importTranscript, onComplete, startBatchRecording]);

  return null;
}
