import { useCallback } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useNewNoteAndListen } from "./useNewNote";

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
  const handleClick = useNewNoteAndListen();
  const openNew = useTabs((state) => state.openNew);

  const handleConfigure = useCallback(() => {
    openNew({ type: "ai", state: { tab: "transcription" } });
  }, [openNew]);

  const button = (
    <button
      type="button"
      onClick={handleClick}
      disabled={isDisabled}
      className={cn([
        "inline-flex items-center justify-center rounded-full text-xs font-medium",
        "bg-stone-800 text-white",
        "hover:bg-stone-700",
        "gap-1.5",
        "h-7 px-3",
        "disabled:pointer-events-none disabled:opacity-50",
      ])}
    >
      <RecordingIcon />
      <span className="whitespace-nowrap">New meeting</span>
    </button>
  );

  if (!warningMessage) {
    return button;
  }

  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
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
  );
}
