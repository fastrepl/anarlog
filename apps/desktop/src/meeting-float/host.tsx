import {
  commands as windowsCommands,
  events as windowsEvents,
} from "@hypr/plugin-windows";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { listenerStore } from "~/store/zustand/listener/instance";

const FLOATING_WINDOW = { type: "floating" } as const;

export const FLOATING_MEETING_PANEL_MARGIN = 4;

const FLOATING_MEETING_PANEL_PANE_SIZE = { width: 116, height: 40 } as const;

export const FLOATING_MEETING_PANEL_SIZE = withPanelMargin(
  FLOATING_MEETING_PANEL_PANE_SIZE,
  FLOATING_MEETING_PANEL_MARGIN,
);

type ListenerState = ReturnType<typeof listenerStore.getState>;
type FloatingRouteState = {
  sessionId: string;
  amplitude: number;
  degraded: boolean;
};

export function FloatingMeetingWindowHost() {
  useMountEffect(() => {
    let routeState = getFloatingRouteState(listenerStore.getState());
    let syncQueued = false;
    let cancelled = false;
    let shownSessionId: string | null = null;
    let nativeCommandsUnavailable = false;
    const unlisteners: Array<() => void> = [];

    const sync = async () => {
      if (nativeCommandsUnavailable && routeState) {
        return;
      }

      const nextShownSessionId = await syncFloatingMeetingWindow(
        routeState,
        shownSessionId,
      );
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
      const nextRouteState = getFloatingRouteState(state);
      const previousRouteState = getFloatingRouteState(previousState);

      if (isSameFloatingRouteState(nextRouteState, previousRouteState)) {
        return;
      }

      routeState = nextRouteState;
      scheduleSync();
    });

    return () => {
      cancelled = true;
      unsubscribe();
      unlisteners.forEach((unlisten) => unlisten());
      void hideFloatingMeetingPanel();
    };
  });

  return null;
}

function getFloatingRouteState(
  state: ListenerState,
  sessionId?: string,
): FloatingRouteState | null {
  if (state.live.status !== "active" && state.live.status !== "finalizing") {
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
    degraded: Boolean(state.live.degraded),
  };
}

function isSameFloatingRouteState(
  left: FloatingRouteState | null,
  right: FloatingRouteState | null,
) {
  return (
    left?.sessionId === right?.sessionId &&
    left?.amplitude === right?.amplitude &&
    left?.degraded === right?.degraded
  );
}

async function syncFloatingMeetingWindow(
  routeState: FloatingRouteState | null,
  shownSessionId: string | null,
): Promise<string | null | "unavailable"> {
  if (!routeState) {
    await hideFloatingMeetingPanel();
    return null;
  }

  const ready = await showFloatingMeetingWindow(
    routeState,
    shownSessionId !== routeState.sessionId,
  );
  return ready ? routeState.sessionId : "unavailable";
}

async function showFloatingMeetingWindow(
  routeState: FloatingRouteState,
  shouldShow: boolean,
): Promise<boolean> {
  if (shouldShow) {
    const showResult = await windowsCommands.floatingBarShow();
    if (showResult.status === "error") {
      console.error("Failed to show floating meeting panel:", showResult.error);
      return false;
    }
  }

  const updateResult = await windowsCommands.floatingBarUpdate({
    amplitude: routeState.amplitude,
    degraded: routeState.degraded,
  });
  if (updateResult.status === "error") {
    console.error(
      "Failed to update floating meeting panel:",
      updateResult.error,
    );
    return false;
  }

  return true;
}

export async function openFloatingMeetingPanel(sessionId?: string) {
  const routeState = getFloatingRouteState(listenerStore.getState(), sessionId);

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

export async function resizeFloatingMeetingPanel() {
  const size = FLOATING_MEETING_PANEL_SIZE;
  const result = await windowsCommands.windowSetFrameAnimated(
    FLOATING_WINDOW,
    "BottomRight",
    size.width,
    size.height,
  );

  if (result.status === "error") {
    console.error("Failed to resize floating meeting panel:", result.error);
  }
}

function withPanelMargin(
  size: { width: number; height: number },
  margin: number,
) {
  return {
    width: size.width + margin * 2,
    height: size.height + margin * 2,
  };
}
