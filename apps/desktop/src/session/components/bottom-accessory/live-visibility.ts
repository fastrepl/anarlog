import {
  getLiveCaptureUiMode,
  type LiveSessionStatus,
} from "~/store/zustand/listener/general-shared";

type LiveTranscriptAccessoryState = {
  status: LiveSessionStatus;
  sessionId: string | null;
  requestedLiveTranscription: boolean | null;
  liveTranscriptionActive: boolean | null;
};

export function shouldShowLiveTranscriptAccessory(
  live: LiveTranscriptAccessoryState,
) {
  if (!live.sessionId) {
    return false;
  }

  if (live.status === "finalizing") {
    return true;
  }

  if (live.status !== "active") {
    return false;
  }

  return getLiveCaptureUiMode(live) === "live";
}
