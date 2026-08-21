import { commands as windowsCommands } from "@anlg/plugin-windows";

import type { FloatingRouteState, LiveCaptionRouteState } from "./route-state";

let sentTranscriptSessionId: string | null = null;
let sentTranscriptBubbles: FloatingRouteState["transcriptBubbles"] | null =
  null;

export function createFloatingMeetingWindowSynchronizer() {
  let desiredRouteState: FloatingRouteState | null = null;
  let desiredRevision = 0;
  let appliedRevision = 0;
  let shownSessionId: string | null = null;
  let appliedRouteState: FloatingRouteState | null = null;
  let nativeCommandsUnavailable = false;
  let running: Promise<void> | null = null;
  let disposeRequested = false;
  let disposed = false;
  const idleWaiters: Array<() => void> = [];

  const resolveIdleWaiters = () => {
    idleWaiters.splice(0).forEach((resolve) => resolve());
  };
  const drain = async () => {
    while (appliedRevision < desiredRevision) {
      const revision = desiredRevision;
      const routeState = desiredRouteState;

      if (nativeCommandsUnavailable && routeState) {
        appliedRevision = revision;
        continue;
      }

      let nextShownSessionId: string | null | "unavailable";
      try {
        nextShownSessionId = await syncFloatingMeetingWindow(
          routeState,
          shownSessionId,
          appliedRouteState,
          () => revision === desiredRevision || desiredRouteState !== null,
        );
      } catch (error) {
        console.error("Failed to synchronize floating meeting panel:", error);
        if (revision === desiredRevision) {
          appliedRevision = revision;
        }
        continue;
      }
      if (revision !== desiredRevision) {
        if (desiredRouteState && nextShownSessionId !== "unavailable") {
          shownSessionId = nextShownSessionId;
          appliedRouteState = routeState;
        }
        continue;
      }

      appliedRevision = revision;
      if (nextShownSessionId === "unavailable") {
        nativeCommandsUnavailable = true;
      } else {
        shownSessionId = nextShownSessionId;
        appliedRouteState = routeState;
      }
    }

    if (disposeRequested) {
      disposed = true;
    }
  };
  const schedule = () => {
    if (running || disposed) {
      return;
    }

    running = drain().finally(() => {
      running = null;
      if (appliedRevision < desiredRevision) {
        schedule();
        return;
      }
      resolveIdleWaiters();
    });
  };
  const waitForIdle = () => {
    if (!running && appliedRevision >= desiredRevision) {
      return Promise.resolve();
    }

    return new Promise<void>((resolve) => {
      idleWaiters.push(resolve);
    });
  };

  return {
    update(routeState: FloatingRouteState | null) {
      if (disposeRequested) {
        return;
      }
      desiredRouteState = routeState;
      desiredRevision += 1;
      schedule();
    },
    dispose() {
      if (!disposeRequested) {
        disposeRequested = true;
        desiredRouteState = null;
        desiredRevision += 1;
        schedule();
      }
      return waitForIdle();
    },
  };
}

export async function syncFloatingMeetingWindow(
  routeState: FloatingRouteState | null,
  shownSessionId: string | null,
  appliedRouteState: FloatingRouteState | null,
  shouldContinue: () => boolean,
): Promise<string | null | "unavailable"> {
  if (!shouldContinue()) {
    return null;
  }

  if (!routeState) {
    await hideFloatingMeetingPanel();
    return null;
  }

  const ready = await showFloatingMeetingWindow(
    routeState,
    shownSessionId !== routeState.sessionId,
    shouldContinue,
    appliedRouteState,
  );
  if (!shouldContinue()) {
    await hideFloatingMeetingPanel();
    return null;
  }

  return ready ? routeState.sessionId : "unavailable";
}

export async function showFloatingMeetingWindow(
  routeState: FloatingRouteState,
  shouldShow: boolean,
  shouldContinue: () => boolean = () => true,
  appliedRouteState: FloatingRouteState | null = null,
): Promise<boolean> {
  if (!shouldContinue()) {
    return false;
  }

  if (shouldShow) {
    const showResult = await windowsCommands.floatingBarShow();
    if (!shouldContinue()) {
      await hideFloatingMeetingPanel();
      return false;
    }

    if (showResult.status === "error") {
      console.error("Failed to show floating meeting panel:", showResult.error);
      return false;
    }
  }

  const amplitudeOnly =
    !shouldShow &&
    appliedRouteState !== null &&
    isAmplitudeOnlyFloatingRouteUpdate(appliedRouteState, routeState);
  const shouldSendTranscript =
    !amplitudeOnly &&
    (shouldShow ||
      sentTranscriptSessionId !== routeState.sessionId ||
      sentTranscriptBubbles !== routeState.transcriptBubbles);

  const updateResult = amplitudeOnly
    ? await windowsCommands.floatingBarUpdateAmplitude(routeState.amplitude)
    : await windowsCommands.floatingBarUpdate({
        amplitude: routeState.amplitude,
        title: routeState.title,
        status: routeState.status,
        colorScheme: routeState.colorScheme,
        opacity: routeState.opacity,
        liveCaptionOpacity: routeState.liveCaptionOpacity,
        liveCaptionWidth: routeState.liveCaptionWidth,
        liveCaptionLineCount: routeState.liveCaptionLineCount,
        liveCaptionPosition: routeState.liveCaptionPosition,
        liveCaptionMinimized: routeState.liveCaptionMinimized,
        liveCaptionToggleVisible: routeState.liveCaptionToggleVisible,
        transcriptBubbles: shouldSendTranscript
          ? routeState.transcriptBubbles
          : null,
      });
  if (!shouldContinue()) {
    await hideFloatingMeetingPanel();
    return false;
  }

  if (updateResult.status === "error") {
    console.error(
      "Failed to update floating meeting panel:",
      updateResult.error,
    );
    return false;
  }

  if (shouldSendTranscript) {
    sentTranscriptSessionId = routeState.sessionId;
    sentTranscriptBubbles = routeState.transcriptBubbles;
  }

  return true;
}

function isAmplitudeOnlyFloatingRouteUpdate(
  previousState: FloatingRouteState,
  nextState: FloatingRouteState,
) {
  return (
    previousState.amplitude !== nextState.amplitude &&
    previousState.sessionId === nextState.sessionId &&
    previousState.title === nextState.title &&
    previousState.status === nextState.status &&
    previousState.colorScheme === nextState.colorScheme &&
    previousState.opacity === nextState.opacity &&
    previousState.liveCaptionOpacity === nextState.liveCaptionOpacity &&
    previousState.liveCaptionWidth === nextState.liveCaptionWidth &&
    previousState.liveCaptionLineCount === nextState.liveCaptionLineCount &&
    previousState.liveCaptionPosition === nextState.liveCaptionPosition &&
    previousState.liveCaptionMinimized === nextState.liveCaptionMinimized &&
    previousState.liveCaptionToggleVisible ===
      nextState.liveCaptionToggleVisible &&
    previousState.transcriptBubbles === nextState.transcriptBubbles
  );
}

export async function hideFloatingMeetingPanel() {
  sentTranscriptSessionId = null;
  sentTranscriptBubbles = null;
  const result = await windowsCommands.floatingBarHide();
  if (result.status === "error") {
    console.error("Failed to hide floating meeting panel:", result.error);
  }
}

export async function hideLiveCaptionPanel() {
  const result = await windowsCommands.liveCaptionHide();
  if (result.status === "error") {
    console.error("Failed to hide live caption panel:", result.error);
  }
}

export function createLiveCaptionWindowSynchronizer() {
  let desiredRouteState: LiveCaptionRouteState | null = null;
  let desiredRevision = 0;
  let appliedRevision = 0;
  let shownSessionId: string | null = null;
  let nativeCommandsUnavailable = false;
  let running: Promise<void> | null = null;
  let disposeRequested = false;
  let disposed = false;
  const idleWaiters: Array<() => void> = [];

  const resolveIdleWaiters = () => {
    idleWaiters.splice(0).forEach((resolve) => resolve());
  };
  const drain = async () => {
    while (appliedRevision < desiredRevision) {
      const revision = desiredRevision;
      const routeState = desiredRouteState;

      if (nativeCommandsUnavailable && routeState) {
        appliedRevision = revision;
        continue;
      }

      let nextShownSessionId: string | null | "unavailable";
      try {
        nextShownSessionId = await syncLiveCaptionWindow(
          routeState,
          shownSessionId,
          () => revision === desiredRevision || desiredRouteState !== null,
        );
      } catch (error) {
        console.error("Failed to synchronize live caption panel:", error);
        if (revision === desiredRevision) {
          appliedRevision = revision;
        }
        continue;
      }
      if (revision !== desiredRevision) {
        if (desiredRouteState && nextShownSessionId !== "unavailable") {
          shownSessionId = nextShownSessionId;
        }
        continue;
      }

      appliedRevision = revision;
      if (nextShownSessionId === "unavailable") {
        nativeCommandsUnavailable = true;
      } else {
        shownSessionId = nextShownSessionId;
      }
    }

    if (disposeRequested) {
      disposed = true;
    }
  };
  const schedule = () => {
    if (running || disposed) {
      return;
    }

    running = drain().finally(() => {
      running = null;
      if (appliedRevision < desiredRevision) {
        schedule();
        return;
      }
      resolveIdleWaiters();
    });
  };
  const waitForIdle = () => {
    if (!running && appliedRevision >= desiredRevision) {
      return Promise.resolve();
    }

    return new Promise<void>((resolve) => {
      idleWaiters.push(resolve);
    });
  };

  return {
    update(routeState: LiveCaptionRouteState | null) {
      if (disposeRequested) {
        return;
      }
      desiredRouteState = routeState;
      desiredRevision += 1;
      schedule();
    },
    dispose() {
      if (!disposeRequested) {
        disposeRequested = true;
        desiredRouteState = null;
        desiredRevision += 1;
        schedule();
      }
      return waitForIdle();
    },
  };
}

export async function syncLiveCaptionWindow(
  routeState: LiveCaptionRouteState | null,
  shownSessionId: string | null,
  shouldContinue: () => boolean,
): Promise<string | null | "unavailable"> {
  if (!shouldContinue()) {
    return null;
  }

  if (!routeState) {
    await hideLiveCaptionPanel();
    return null;
  }

  const ready = await showLiveCaptionWindow(
    routeState,
    shownSessionId !== routeState.sessionId,
    shouldContinue,
  );
  if (!shouldContinue()) {
    await hideLiveCaptionPanel();
    return null;
  }

  return ready ? routeState.sessionId : "unavailable";
}

export async function showLiveCaptionWindow(
  routeState: LiveCaptionRouteState,
  shouldShow: boolean,
  shouldContinue: () => boolean = () => true,
): Promise<boolean> {
  if (!shouldContinue()) {
    return false;
  }

  if (shouldShow) {
    const showResult = await windowsCommands.liveCaptionShow();
    if (!shouldContinue()) {
      await hideLiveCaptionPanel();
      return false;
    }

    if (showResult.status === "error") {
      console.error("Failed to show live caption panel:", showResult.error);
      return false;
    }
  }

  const updateResult = await windowsCommands.liveCaptionUpdate({
    text: routeState.text,
    opacity: routeState.opacity,
    width: routeState.width,
    lineCount: routeState.lineCount,
    position: routeState.position,
    minimized: routeState.minimized,
  });
  if (!shouldContinue()) {
    await hideLiveCaptionPanel();
    return false;
  }

  if (updateResult.status === "error") {
    console.error("Failed to update live caption panel:", updateResult.error);
    return false;
  }

  return true;
}
