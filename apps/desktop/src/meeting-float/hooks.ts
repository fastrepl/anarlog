import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { useStore } from "zustand";

import {
  events as listenerEvents,
  type CaptureDataEvent,
  type CaptureLifecycleEvent,
  type LiveTranscriptDelta,
  type LiveTranscriptSegmentDelta,
} from "@hypr/plugin-transcription";

import { getSessionEvent } from "~/session/utils";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import * as main from "~/store/tinybase/store/main";
import {
  getLiveCaptureUiMode,
  setLiveState,
  updateLiveAmplitude,
} from "~/store/zustand/listener/general-shared";
import { listenerStore } from "~/store/zustand/listener/instance";
import { mergeRenderedAndLiveSegments, type Segment } from "~/stt/live-segment";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";

const EMPTY_SEGMENTS: Segment[] = [];

export function useMirrorFloatingCaptureState(
  sessionId: string,
  initialShowTranscriptTab: boolean,
) {
  useMountEffect(() => {
    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    seedFloatingCaptureState(sessionId, initialShowTranscriptTab);

    void Promise.all([
      listenerEvents.captureLifecycleEvent.listen(({ payload }) =>
        applyFloatingCaptureLifecycle(sessionId, payload),
      ),
      listenerEvents.captureDataEvent.listen(({ payload }) =>
        applyFloatingCaptureData(sessionId, payload),
      ),
    ]).then((nextUnlisteners) => {
      if (cancelled) {
        nextUnlisteners.forEach((unlisten) => unlisten());
        return;
      }

      unlisteners.push(...nextUnlisteners);
    });

    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
      clearFloatingCaptureState(sessionId);
    };
  });
}

export function useFloatingMeetingTitle(sessionId: string) {
  const title = main.UI.useCell(
    "sessions",
    sessionId,
    "title",
    main.STORE_ID,
  ) as string | undefined;
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  ) as string | undefined;

  return useMemo(() => {
    const eventTitle = getSessionEvent({ event_json: eventJson })?.title;
    return eventTitle || title || "Live meeting";
  }, [eventJson, title]);
}

export function useFloatingShowTranscriptTab(
  sessionId: string,
  fallback: boolean,
) {
  return useStore(listenerStore, (state) => {
    if (state.live.sessionId !== sessionId) {
      return fallback;
    }

    return (
      getLiveCaptureUiMode({
        requestedLiveTranscription: state.live.requestedLiveTranscription,
        liveTranscriptionActive: state.live.liveTranscriptionActive,
      }) === "live"
    );
  });
}

export function useFloatingWaveformState() {
  const amplitude = useStore(listenerStore, (state) => state.live.amplitude);
  const degraded = useStore(listenerStore, (state) => state.live.degraded);

  return { amplitude, degraded };
}

export function useFloatingTranscriptSegments(sessionId: string): Segment[] {
  const store = main.UI.useStore(main.STORE_ID);
  const transcriptIds =
    main.UI.useSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId,
      main.STORE_ID,
    ) ?? [];
  main.UI.useTable("transcripts", main.STORE_ID);
  main.UI.useTable("mapping_session_participant", main.STORE_ID);
  main.UI.useTable("humans", main.STORE_ID);
  main.UI.useValue("user_id", main.STORE_ID);
  const liveSegments = useStore(listenerStore, (state): Segment[] =>
    state.live.sessionId === sessionId ? state.liveSegments : EMPTY_SEGMENTS,
  );

  const request =
    store && transcriptIds.length > 0
      ? buildRenderTranscriptRequestFromStore(store, transcriptIds)
      : null;

  const { data: renderedSegments = [] } = useQuery({
    queryKey: ["floating-meeting-transcript-segments", sessionId, request],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
  });

  return useMemo(() => {
    return mergeRenderedAndLiveSegments(renderedSegments, liveSegments);
  }, [liveSegments, renderedSegments]);
}

export function useFloatingTranscriptLabels(segments: Segment[]) {
  const store = main.UI.useStore(main.STORE_ID);
  const labelContext = useMemo(
    () => (store ? defaultRenderLabelContext(store) : undefined),
    [store],
  );
  const speakerLabelManager = useMemo(() => {
    if (!store) {
      return new SpeakerLabelManager();
    }

    return SpeakerLabelManager.fromSegments(segments, labelContext);
  }, [labelContext, segments, store]);

  return useMemo(
    () => ({ labelContext, speakerLabelManager }),
    [labelContext, speakerLabelManager],
  );
}

function seedFloatingCaptureState(
  sessionId: string,
  initialShowTranscriptTab: boolean,
) {
  setLiveState(listenerStore.setState, (live) => {
    live.status = "active";
    live.loading = false;
    live.loadingPhase = "idle";
    live.sessionId = sessionId;
    live.degraded = null;
    live.requestedLiveTranscription = initialShowTranscriptTab;
    live.liveTranscriptionActive = initialShowTranscriptTab;
  });
}

function clearFloatingCaptureState(sessionId: string) {
  const state = listenerStore.getState();
  if (state.live.sessionId !== sessionId) {
    return;
  }

  setLiveState(listenerStore.setState, (live) => {
    live.status = "inactive";
    live.loading = false;
    live.loadingPhase = "idle";
    live.sessionId = null;
    live.amplitude = { mic: 0, speaker: 0 };
    live.muted = false;
    live.degraded = null;
    live.requestedLiveTranscription = null;
    live.liveTranscriptionActive = null;
  });
  state.resetTranscript();
}

function applyFloatingCaptureLifecycle(
  sessionId: string,
  payload: CaptureLifecycleEvent,
) {
  if (payload.session_id !== sessionId) {
    return;
  }

  if (payload.type === "started") {
    setLiveState(listenerStore.setState, (live) => {
      live.status = "active";
      live.loading = false;
      live.loadingPhase = "idle";
      live.sessionId = sessionId;
      live.degraded = payload.degraded ?? null;
      live.requestedLiveTranscription = payload.requested_live_transcription;
      live.liveTranscriptionActive = payload.live_transcription_active;
    });
    return;
  }

  if (payload.type === "finalizing") {
    setLiveState(listenerStore.setState, (live) => {
      live.status = "finalizing";
      live.loading = true;
      live.sessionId = sessionId;
    });
    return;
  }

  clearFloatingCaptureState(sessionId);
}

function applyFloatingCaptureData(
  sessionId: string,
  payload: CaptureDataEvent,
) {
  if (payload.session_id !== sessionId) {
    return;
  }

  if (payload.type === "audio_amplitude") {
    setLiveState(listenerStore.setState, (live) => {
      updateLiveAmplitude(live, payload.mic, payload.speaker);
    });
    return;
  }

  if (payload.type === "mic_muted") {
    setLiveState(listenerStore.setState, (live) => {
      live.muted = payload.value;
    });
    return;
  }

  if (payload.type === "transcript_delta") {
    listenerStore
      .getState()
      .handleTranscriptDelta(
        sessionId,
        payload.delta as unknown as LiveTranscriptDelta,
      );
    return;
  }

  listenerStore
    .getState()
    .handleTranscriptSegmentDelta(
      payload.delta as unknown as LiveTranscriptSegmentDelta,
    );
}
