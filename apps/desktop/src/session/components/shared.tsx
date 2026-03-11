import { useMemo } from "react";

import { Button } from "@hypr/ui/components/ui/button";

import { computeCurrentNoteTab } from "./compute-note-tab";

import { useAITaskTask } from "~/ai/hooks";
import { useNetwork } from "~/contexts/network";
import { countTranscriptWords } from "~/services/enhancer/eligibility";
import * as main from "~/store/tinybase/store/main";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import type { Tab } from "~/store/zustand/tabs/schema";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { MIN_WORDS_FOR_MEANINGFUL_TRANSCRIPT } from "~/stt/thresholds";
import { useSTTConnection } from "~/stt/useSTTConnection";

export { computeCurrentNoteTab } from "./compute-note-tab";

export function useHasTranscript(sessionId: string): boolean {
  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );
  const store = main.UI.useStore(main.STORE_ID);

  return useMemo(() => {
    if (!store || !transcriptIds?.length) {
      return false;
    }

    return (
      countTranscriptWords(transcriptIds, store) >=
      MIN_WORDS_FOR_MEANINGFUL_TRANSCRIPT
    );
  }, [store, transcriptIds]);
}

export function useCurrentNoteTab(
  tab: Extract<Tab, { type: "sessions" }>,
): EditorView {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const isListenerStarting = useListener(
    (state) =>
      state.live.loading &&
      state.live.sessionId === tab.id &&
      state.live.status === "inactive",
  );
  const isListenerActive =
    sessionMode === "active" ||
    sessionMode === "finalizing" ||
    isListenerStarting;
  const hasTranscript = useHasTranscript(tab.id);

  const enhancedNoteIds = main.UI.useSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    tab.id,
    main.STORE_ID,
  );

  return useMemo(
    () =>
      computeCurrentNoteTab(
        tab.state.view ?? null,
        isListenerActive,
        hasTranscript,
        enhancedNoteIds ?? [],
      ),
    [tab.state.view, isListenerActive, hasTranscript, enhancedNoteIds],
  );
}

export function RecordingIcon() {
  return <div className="size-3 rounded-full bg-red-500" />;
}

export function useListenButtonState(sessionId: string) {
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const lastError = useListener((state) => state.live.lastError);
  const active = sessionMode === "active" || sessionMode === "finalizing";
  const batching = sessionMode === "running_batch";

  const taskId = createTaskId(sessionId, "enhance");
  const { status } = useAITaskTask(taskId, "enhance");
  const generating = status === "generating";
  const { conn: sttConnection, local, isLocalModel } = useSTTConnection();
  const { isOnline } = useNetwork();

  const localServerStatus = local.data?.status ?? "unavailable";
  const isLocalServerLoading = localServerStatus === "loading";
  const isLocalModelNotDownloaded = localServerStatus === "not_downloaded";

  const isOfflineWithCloudModel = !isOnline && !isLocalModel;

  const shouldRender = !active && !generating;
  const isDisabled =
    !sttConnection ||
    batching ||
    isLocalServerLoading ||
    isLocalModelNotDownloaded ||
    isOfflineWithCloudModel;

  let warningMessage = "";
  if (lastError) {
    warningMessage = `Session failed: ${lastError}`;
  } else if (isLocalModelNotDownloaded) {
    warningMessage = "Selected model is not downloaded.";
  } else if (isLocalServerLoading) {
    warningMessage = "Local STT server is starting up...";
  } else if (isOfflineWithCloudModel) {
    warningMessage = "You're offline. Use on-device models to continue.";
  } else if (!sttConnection) {
    warningMessage = "Transcription model not available.";
  } else if (batching) {
    warningMessage = "Batch transcription in progress.";
  }

  return {
    shouldRender,
    isDisabled,
    warningMessage,
  };
}

export function ActionableTooltipContent({
  message,
  action,
}: {
  message: string;
  action?: {
    label: string;
    handleClick: () => void;
  };
}) {
  return (
    <div className="flex flex-row items-center gap-3">
      <p className="text-xs">{message}</p>
      {action && (
        <Button
          size="sm"
          variant="outline"
          className="rounded-md text-black"
          onClick={action.handleClick}
        >
          {action.label}
        </Button>
      )}
    </div>
  );
}
