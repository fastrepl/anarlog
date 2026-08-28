import * as stylex from "@stylexjs/stylex";
import { useMemo } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

import { computeCurrentNoteTab } from "./compute-note-tab";

import { extractPlainText } from "~/search/contexts/engine/utils";
import {
  useEnhancedNote,
  useEnhancedNoteRecords,
  useSession,
  useSessionHasTranscript,
} from "~/session/queries";
import type { SessionMode } from "~/store/zustand/listener/general";
import type { Tab } from "~/store/zustand/tabs/schema";
import { type EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

export { computeCurrentNoteTab } from "./compute-note-tab";

export function useHasTranscript(sessionId: string): boolean {
  return useSessionHasTranscript(sessionId);
}

export function hasStoredNoteContent(value: unknown): boolean {
  return extractPlainText(value).trim().length > 0;
}

export function useCurrentNoteHasContent(
  sessionId: string,
  currentView: EditorView,
): boolean {
  const hasTranscript = useHasTranscript(sessionId);
  const rawMd = useSession(sessionId)?.raw_md;
  const enhancedNoteId = currentView.type === "enhanced" ? currentView.id : "";
  const enhancedContent = useEnhancedNote(enhancedNoteId)?.content;

  if (currentView.type === "enhanced") {
    return hasStoredNoteContent(enhancedContent);
  }

  if (currentView.type === "transcript") {
    return hasTranscript;
  }

  return hasStoredNoteContent(rawMd);
}

export function useCurrentNoteTab(
  tab: Extract<Tab, { type: "sessions" }>,
  { audioExists = false }: { audioExists?: boolean } = {},
): EditorView {
  const sessionMode = useListener((state) => state.getSessionMode(tab.id));
  const isLiveSessionActive = sessionMode === "active";
  const canShowTranscript = useCanShowTranscript(tab.id, { audioExists });

  const enhancedNoteIds = useEnhancedNoteRecords(tab.id).map((note) => note.id);

  return useMemo(() => {
    return computeCurrentNoteTab(
      tab.state.view ?? null,
      isLiveSessionActive,
      enhancedNoteIds,
      canShowTranscript,
    );
  }, [tab.state.view, isLiveSessionActive, enhancedNoteIds, canShowTranscript]);
}

export function useCanShowTranscript(
  sessionId: string,
  { audioExists = false }: { audioExists?: boolean } = {},
): boolean {
  const hasTranscript = useHasTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener((state) => state.batch[sessionId]?.error);
  const hasLiveSegments = useListener(
    (state) =>
      state.live.sessionId === sessionId && state.liveSegments.length > 0,
  );

  return getCanShowTranscript({
    audioExists,
    batchError: Boolean(batchError),
    hasLiveSegments,
    hasTranscript,
    sessionMode,
  });
}

export function getCanShowTranscript({
  audioExists = false,
  batchError = false,
  hasLiveSegments = false,
  hasTranscript,
  sessionMode,
}: {
  audioExists?: boolean;
  batchError?: boolean;
  hasLiveSegments?: boolean;
  hasTranscript: boolean;
  sessionMode: SessionMode;
}): boolean {
  const isLiveCapture =
    sessionMode === "active" || sessionMode === "finalizing";

  return (
    hasTranscript ||
    (audioExists && !isLiveCapture) ||
    isLiveCapture ||
    hasLiveSegments ||
    sessionMode === "running_batch" ||
    Boolean(batchError)
  );
}

export function RecordingIcon() {
  return (
    <span {...stylex.props(styles.recordingIcon)}>
      <span {...stylex.props(styles.recordingPulse)} />
      <span {...stylex.props(styles.recordingDot)} />
    </span>
  );
}

export function useListenButtonState(sessionId: string) {
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const lastError = useListener((state) => state.live.lastError);
  const lastErrorSessionId = useListener(
    (state) => state.live.lastErrorSessionId,
  );
  const lastErrorIsAudioRelated = useListener(
    (state) => state.live.lastErrorIsAudioRelated,
  );
  const active = sessionMode === "active" || sessionMode === "finalizing";
  const batching = sessionMode === "running_batch";

  const shouldRender = !active;
  const isDisabled = batching;

  let warningMessage = "";
  let recoverySettingsTab: "permissions" | null = null;
  if (lastError && lastErrorSessionId === sessionId) {
    warningMessage = `Session failed: ${lastError}`;
    recoverySettingsTab = lastErrorIsAudioRelated ? "permissions" : null;
  } else if (batching) {
    warningMessage = "Batch transcription in progress.";
  }

  return {
    shouldRender,
    isDisabled,
    warningMessage,
    recoverySettingsTab,
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
    <div {...stylex.props(styles.tooltip)}>
      <p {...stylex.props(styles.tooltipMessage)}>{message}</p>
      {action && (
        <Button
          size="sm"
          variant="outline"
          sx={styles.tooltipAction}
          onClick={action.handleClick}
        >
          {action.label}
        </Button>
      )}
    </div>
  );
}

const ping = stylex.keyframes({
  "75%, 100%": {
    opacity: 0,
    transform: "scale(2)",
  },
});

const styles = stylex.create({
  recordingDot: {
    backgroundColor: "#ef4444",
    borderRadius: radii.full,
    height: "0.5rem",
    position: "relative",
    width: "0.5rem",
  },
  recordingIcon: {
    alignItems: "center",
    display: "flex",
    height: "0.75rem",
    justifyContent: "center",
    position: "relative",
    width: "0.75rem",
  },
  recordingPulse: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: ping,
    animationTimingFunction: "cubic-bezier(0, 0, 0.2, 1)",
    backgroundColor: "rgb(239 68 68 / 0.4)",
    borderRadius: radii.full,
    height: "0.625rem",
    position: "absolute",
    width: "0.625rem",
  },
  tooltip: {
    alignItems: "center",
    display: "flex",
    flexDirection: "row",
    gap: "0.75rem",
  },
  tooltipAction: {
    borderRadius: radii.md,
    color: colors.foreground,
  },
  tooltipMessage: {
    fontSize: "0.75rem",
  },
});
