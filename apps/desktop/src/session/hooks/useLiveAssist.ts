import { useEffect, useRef } from "react";

import { useLanguageModel } from "~/ai/hooks";
import { hydrateSessionContext } from "~/chat/context/session-context-hydrator";
import { useShell } from "~/contexts/shell";
import {
  enqueueLiveAssistKind,
  registerLiveAssistContextBuilder,
  resetLiveAssistJobs,
} from "~/session/hooks/live-assist-job";
import {
  buildLiveAssistWindow,
  buildSummarizeSoFarInput,
  LIVE_ASSIST_CATCH_UP_WINDOW_MS,
  LIVE_ASSIST_SUMMARIZE_SO_FAR_MAX_CHARS,
  LIVE_ASSIST_WINDOW_MAX_CHARS,
  type LiveAssistKind,
} from "~/session/insights/live-assist";
import { useConfigValue } from "~/shared/config";
import { useOwnerUserId } from "~/shared/owner-user";
import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useLiveAssistStore } from "~/store/zustand/live-assist";
import { useLiveAssistPanelTab } from "~/store/zustand/live-assist/panel-tab";
import { useListener } from "~/stt/contexts";

const LIVE_ASSIST_TICK_INTERVAL_MS = 60_000;
// The full-meeting summary is the most expensive kind and the least
// time-sensitive one to refresh; a pragmatic v1 default keeps it well below
// the 60s cadence of the other three kinds instead of a configurable value.
const SUMMARIZE_SO_FAR_MIN_INTERVAL_MS = 5 * 60_000;
const ROLLING_KINDS: readonly LiveAssistKind[] = [
  "catch_up",
  "action_items",
  "follow_up",
];

// Proactively enqueues Live Assist suggestions on a timer while a session is
// actively recording with live transcription. Mount once per active session
// (see `NoteInputContent`) so the timer and its in-flight jobs share that
// component's lifetime.
export function useLiveAssist(sessionId: string) {
  const enabled = useConfigValue("live_assist_enabled");
  const language = useConfigValue("ai_language") || "en";
  const model = useLanguageModel("chat");
  const ownerUserId = useOwnerUserId();
  const { chat } = useShell();
  const setPanelTab = useLiveAssistPanelTab((state) => state.setActiveTab);

  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const liveCaptureUiMode = useListener((state) =>
    getLiveCaptureUiMode(state.live),
  );
  const liveSegments = useListener((state) => state.liveSegments);

  // Live Assist only makes sense while live transcription is actually
  // feeding the session; a record-only fallback has no text to work from.
  const isSessionLive =
    sessionMode === "active" && liveCaptureUiMode === "live";
  const active = enabled && Boolean(model) && isSessionLive;

  const modelRef = useRef(model);
  modelRef.current = model;
  const languageRef = useRef(language);
  languageRef.current = language;
  const liveSegmentsRef = useRef(liveSegments);
  liveSegmentsRef.current = liveSegments;
  const ownerUserIdRef = useRef(ownerUserId);
  ownerUserIdRef.current = ownerUserId;
  const chatModeRef = useRef(chat.mode);
  chatModeRef.current = chat.mode;
  const sendChatEventRef = useRef(chat.sendEvent);
  sendChatEventRef.current = chat.sendEvent;

  const lastFiredWindowTextRef = useRef<
    Partial<Record<LiveAssistKind, string>>
  >({});
  const lastSummarizeAtMsRef = useRef(0);
  const hasAutoOpenedRef = useRef(false);

  // Builds each kind's freshest input at the moment it is actually its turn
  // to run (not when it was enqueued), so a kind stuck briefly behind
  // another in the FIFO still reads the latest transcript.
  useEffect(() => {
    if (!active) {
      return;
    }

    return registerLiveAssistContextBuilder(sessionId, async (kind) => {
      const currentModel = modelRef.current;
      if (!currentModel) {
        return null;
      }

      if (kind === "summarize_so_far") {
        const context = await hydrateSessionContext(
          sessionId,
          ownerUserIdRef.current ?? undefined,
        );
        const sourceText = buildSummarizeSoFarInput(
          context?.transcript ?? null,
          LIVE_ASSIST_SUMMARIZE_SO_FAR_MAX_CHARS,
        );
        return {
          model: currentModel,
          language: languageRef.current,
          sourceText,
        };
      }

      const nowMs = Date.now();
      const sourceText = buildLiveAssistWindow(
        liveSegmentsRef.current,
        nowMs - LIVE_ASSIST_CATCH_UP_WINDOW_MS,
        nowMs,
        LIVE_ASSIST_WINDOW_MAX_CHARS,
      );
      return {
        model: currentModel,
        language: languageRef.current,
        sourceText,
      };
    });
  }, [active, sessionId]);

  // Opens the right panel and defaults it to the Live Assist tab the first
  // time a session goes live, but only if the user had not already opened
  // chat themselves (panel already open, or floating).
  useEffect(() => {
    if (!active) {
      hasAutoOpenedRef.current = false;
      return;
    }
    if (hasAutoOpenedRef.current) {
      return;
    }
    hasAutoOpenedRef.current = true;
    if (chatModeRef.current === "FloatingClosed") {
      sendChatEventRef.current({ type: "OPEN_RIGHT_PANEL" });
      setPanelTab("live_assist");
    }
  }, [active, setPanelTab]);

  useEffect(() => {
    if (!active) {
      return;
    }

    const tick = () => {
      const nowMs = Date.now();
      const windowText = buildLiveAssistWindow(
        liveSegmentsRef.current,
        nowMs - LIVE_ASSIST_CATCH_UP_WINDOW_MS,
        nowMs,
        LIVE_ASSIST_WINDOW_MAX_CHARS,
      );

      // Anti-waste guard: skip a kind entirely when its source text has not
      // changed since it last fired. The three rolling kinds share the same
      // window, so in practice they fire together or not at all.
      if (windowText.trim()) {
        for (const kind of ROLLING_KINDS) {
          if (lastFiredWindowTextRef.current[kind] === windowText) {
            continue;
          }
          lastFiredWindowTextRef.current[kind] = windowText;
          enqueueLiveAssistKind(sessionId, kind);
        }
      }

      const hasTranscript = liveSegmentsRef.current.length > 0;
      const dueForSummary =
        nowMs - lastSummarizeAtMsRef.current >=
        SUMMARIZE_SO_FAR_MIN_INTERVAL_MS;
      if (hasTranscript && dueForSummary) {
        lastSummarizeAtMsRef.current = nowMs;
        enqueueLiveAssistKind(sessionId, "summarize_so_far");
      }
    };

    const intervalId = setInterval(tick, LIVE_ASSIST_TICK_INTERVAL_MS);
    return () => clearInterval(intervalId);
  }, [active, sessionId]);

  // Leaving live capture (session paused, stopped, or fell back to
  // record-only) aborts in-flight/pending jobs but keeps already-resolved
  // cards visible; only a real end of session clears them (below).
  useEffect(() => {
    if (!active) {
      resetLiveAssistJobs(sessionId);
    }
  }, [active, sessionId]);

  const previousSessionModeRef = useRef(sessionMode);
  useEffect(() => {
    const previousSessionMode = previousSessionModeRef.current;
    previousSessionModeRef.current = sessionMode;
    if (previousSessionMode !== "inactive" && sessionMode === "inactive") {
      resetLiveAssistJobs(sessionId);
      useLiveAssistStore.getState().clearSession(sessionId);
    }
  }, [sessionMode, sessionId]);

  useEffect(() => {
    return () => {
      resetLiveAssistJobs(sessionId);
    };
  }, [sessionId]);
}
