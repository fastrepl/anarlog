import {
  commands as windowsCommands,
  events as windowsEvents,
} from "@hypr/plugin-windows";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { listenerStore } from "~/store/zustand/listener/instance";

type ListenerState = ReturnType<typeof listenerStore.getState>;
type FloatingBarStatus = "recording" | "error";
type FloatingBarColorScheme = "light" | "dark";
type FloatingRouteState = {
  sessionId: string;
  amplitude: number;
  status: FloatingBarStatus;
  colorScheme: FloatingBarColorScheme;
};
type LiveCaptionRouteState = {
  sessionId: string;
  text: string;
  opacity: number;
};

export function FloatingMeetingWindowHost() {
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");

  return (
    <>
      {floatingBarEnabled ? (
        <FloatingMeetingWindowSync />
      ) : (
        <FloatingMeetingWindowDisabled />
      )}
      <LiveCaptionWindowSync />
    </>
  );
}

function FloatingMeetingWindowDisabled() {
  useMountEffect(() => {
    void hideFloatingMeetingPanel();
  });

  return null;
}

function LiveCaptionWindowSync() {
  useMountEffect(() => {
    let routeState = getCurrentLiveCaptionRouteState(listenerStore.getState());
    let syncQueued = false;
    let syncRunning = false;
    let syncRequested = false;
    let cancelled = false;
    let shownSessionId: string | null = null;

    const shouldContinue = () => !cancelled;

    const sync = async () => {
      if (!shouldContinue()) {
        return;
      }

      const nextShownSessionId = await syncLiveCaptionWindow(
        routeState,
        shownSessionId,
        shouldContinue,
      );
      if (!shouldContinue()) {
        await hideLiveCaptionPanel();
        return;
      }

      if (nextShownSessionId === "unavailable") {
        return;
      }

      shownSessionId = nextShownSessionId;
    };

    const runQueuedSync = async () => {
      if (syncRunning) {
        syncRequested = true;
        return;
      }

      syncRunning = true;
      try {
        do {
          syncRequested = false;
          await sync();
        } while (syncRequested && !cancelled);
      } finally {
        syncRunning = false;
      }
    };

    const scheduleSync = () => {
      if (syncQueued) {
        return;
      }

      syncQueued = true;
      queueMicrotask(() => {
        syncQueued = false;
        if (cancelled) {
          return;
        }

        void runQueuedSync();
      });
    };

    scheduleSync();

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      const nextRouteState = getLiveCaptionRouteState(state);
      const previousRouteState = getLiveCaptionRouteState(previousState);

      if (isSameLiveCaptionRouteState(nextRouteState, previousRouteState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    return () => {
      cancelled = true;
      unsubscribe();
      void hideLiveCaptionPanel();
    };
  });

  return null;
}

function FloatingMeetingWindowSync() {
  useMountEffect(() => {
    let routeState = getCurrentFloatingRouteState(listenerStore.getState());
    let syncQueued = false;
    let cancelled = false;
    let shownSessionId: string | null = null;
    let nativeCommandsUnavailable = false;
    const unlisteners: Array<() => void> = [];

    const shouldContinue = () => !cancelled;

    const sync = async () => {
      if (!shouldContinue()) {
        return;
      }

      if (nativeCommandsUnavailable && routeState) {
        return;
      }

      const nextShownSessionId = await syncFloatingMeetingWindow(
        routeState,
        shownSessionId,
        shouldContinue,
      );
      if (!shouldContinue()) {
        await hideFloatingMeetingPanel();
        return;
      }

      if (nextShownSessionId === "unavailable") {
        nativeCommandsUnavailable = true;
        return;
      }

      shownSessionId = nextShownSessionId;
    };

    const scheduleSync = () => {
      if (syncQueued) {
        return;
      }

      syncQueued = true;
      queueMicrotask(() => {
        syncQueued = false;
        if (cancelled) {
          return;
        }

        void sync();
      });
    };

    windowsEvents.floatingBarStop
      .listen(() => {
        void hideFloatingMeetingPanel();
        listenerStore.getState().stop();
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlisteners.push(unlisten);
      });

    windowsEvents.floatingBarOpenMain
      .listen(() => {
        void windowsCommands.windowShow({ type: "main" });
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlisteners.push(unlisten);
      });

    scheduleSync();

    const unsubscribe = listenerStore.subscribe((state, previousState) => {
      const colorScheme = getCurrentFloatingBarColorScheme();
      const nextRouteState = getFloatingRouteState(state, { colorScheme });
      const previousRouteState = getFloatingRouteState(previousState, {
        colorScheme,
      });

      if (isSameFloatingRouteState(nextRouteState, previousRouteState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    const unsubscribeAppliedTheme = subscribeToAppliedTheme(() => {
      const nextRouteState = getCurrentFloatingRouteState(
        listenerStore.getState(),
      );

      if (isSameFloatingRouteState(nextRouteState, routeState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    return () => {
      cancelled = true;
      unsubscribe();
      unsubscribeAppliedTheme();
      unlisteners.forEach((unlisten) => unlisten());
      void hideFloatingMeetingPanel();
    };
  });

  return null;
}

export function getFloatingRouteState(
  state: ListenerState,
  {
    sessionId,
    colorScheme = "dark",
  }: {
    sessionId?: string;
    colorScheme?: FloatingBarColorScheme;
  } = {},
): FloatingRouteState | null {
  if (state.live.status !== "active") {
    return null;
  }

  if (!state.live.sessionId) {
    return null;
  }

  if (sessionId && state.live.sessionId !== sessionId) {
    return null;
  }

  return {
    sessionId: state.live.sessionId,
    amplitude: Math.min(
      Math.hypot(state.live.amplitude.mic, state.live.amplitude.speaker),
      1,
    ),
    status: state.live.degraded || state.live.lastError ? "error" : "recording",
    colorScheme,
  };
}

function getCurrentFloatingRouteState(
  state: ListenerState,
  sessionId?: string,
): FloatingRouteState | null {
  return getFloatingRouteState(state, {
    sessionId,
    colorScheme: getCurrentFloatingBarColorScheme(),
  });
}

export function getLiveCaptionRouteState(
  state: ListenerState,
): LiveCaptionRouteState | null {
  if (state.live.status !== "active") {
    return null;
  }

  if (!state.live.sessionId) {
    return null;
  }

  if (state.live.liveTranscriptionActive !== true) {
    return null;
  }

  const text = state.liveCaptionText.trim();
  if (!text) {
    return null;
  }

  return {
    sessionId: state.live.sessionId,
    text,
    opacity: 0.78,
  };
}

function getCurrentLiveCaptionRouteState(
  state: ListenerState,
): LiveCaptionRouteState | null {
  return getLiveCaptionRouteState(state);
}

function subscribeToAppliedTheme(onStoreChange: () => void) {
  if (
    typeof document === "undefined" ||
    typeof MutationObserver === "undefined"
  ) {
    return () => {};
  }

  const observer = new MutationObserver(onStoreChange);
  observer.observe(document.documentElement, {
    attributeFilter: ["class"],
    attributes: true,
  });
  return () => observer.disconnect();
}

export function getCurrentFloatingBarColorScheme(): FloatingBarColorScheme {
  if (typeof document === "undefined") {
    return "dark";
  }

  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

function isSameFloatingRouteState(
  left: FloatingRouteState | null,
  right: FloatingRouteState | null,
) {
  return (
    left?.sessionId === right?.sessionId &&
    left?.amplitude === right?.amplitude &&
    left?.status === right?.status &&
    left?.colorScheme === right?.colorScheme
  );
}

function isSameLiveCaptionRouteState(
  left: LiveCaptionRouteState | null,
  right: LiveCaptionRouteState | null,
) {
  return (
    left?.sessionId === right?.sessionId &&
    left?.text === right?.text &&
    left?.opacity === right?.opacity
  );
}

async function syncFloatingMeetingWindow(
  routeState: FloatingRouteState | null,
  shownSessionId: string | null,
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
  );
  if (!shouldContinue()) {
    await hideFloatingMeetingPanel();
    return null;
  }

  return ready ? routeState.sessionId : "unavailable";
}

async function syncLiveCaptionWindow(
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

async function showFloatingMeetingWindow(
  routeState: FloatingRouteState,
  shouldShow: boolean,
  shouldContinue: () => boolean = () => true,
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

  const updateResult = await windowsCommands.floatingBarUpdate({
    amplitude: routeState.amplitude,
    status: routeState.status,
    colorScheme: routeState.colorScheme,
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

  return true;
}

async function showLiveCaptionWindow(
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

export async function openFloatingMeetingPanel({
  sessionId,
  enabled,
}: {
  sessionId?: string;
  enabled: boolean;
}) {
  if (!enabled) {
    await hideFloatingMeetingPanel();
    return;
  }

  const routeState = getCurrentFloatingRouteState(
    listenerStore.getState(),
    sessionId,
  );

  if (!routeState) {
    return;
  }

  await showFloatingMeetingWindow(routeState, true);
}

export async function hideFloatingMeetingPanel() {
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
