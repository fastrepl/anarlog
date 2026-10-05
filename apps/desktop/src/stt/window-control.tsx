import { emitTo, listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

import { commands as listenerCommands } from "@anlg/plugin-transcription";
import { getCurrentWebviewWindowLabel } from "@anlg/plugin-windows";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { useListener } from "./contexts";
import { useStartListeningWithBatchOverride } from "./useStartListeningWithBatchOverride";

import { listenerStore } from "~/store/zustand/listener/instance";

const LISTENER_CONTROL_EVENT = "anlg:listener-control";
// The main webview may be dead or reloading; if it hasn't stopped the
// capture by then, stop it natively.
const MAIN_STOP_FALLBACK_DELAY_MS = 1_500;

type ListenerControlAction = "start" | "stop";

type ListenerControlRequest = {
  action: ListenerControlAction;
  requestId: string;
  sessionId: string;
};

export function isMainWebviewWindow() {
  return getCurrentWebviewWindowLabel() === "main";
}

export async function requestMainListenerControl(
  action: ListenerControlAction,
  sessionId: string,
) {
  const request = emitTo("main", LISTENER_CONTROL_EVENT, {
    action,
    requestId: crypto.randomUUID(),
    sessionId,
  } satisfies ListenerControlRequest);

  if (action !== "stop") {
    await request;
    return;
  }

  await request.catch((error) => {
    console.error("Failed to request stop from the main window:", error);
  });
  await new Promise((resolve) =>
    setTimeout(resolve, MAIN_STOP_FALLBACK_DELAY_MS),
  );
  await stopCaptureForSession(sessionId);
}

async function stopCaptureForSession(sessionId: string) {
  const result = await listenerCommands.stopCaptureForSession(sessionId);
  if (result.status === "error") {
    console.error("Failed to stop capture:", result.error);
  }
}

export function MainListenerControlBridge() {
  const [requests, setRequests] = useState<ListenerControlRequest[]>([]);

  useMountEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void listen<ListenerControlRequest>(LISTENER_CONTROL_EVENT, (event) => {
      if (!active) {
        return;
      }

      setRequests((current) => [...current, event.payload]);
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    return () => {
      active = false;
      unlisten?.();
    };
  });

  const handleRequestHandled = useCallback((requestId: string) => {
    setRequests((current) => {
      if (current[0]?.requestId === requestId) {
        return current.slice(1);
      }

      return current.filter((request) => request.requestId !== requestId);
    });
  }, []);

  const request = requests[0];
  if (!request) {
    return null;
  }

  return (
    <MainListenerControlRequestRunner
      request={request}
      onHandled={handleRequestHandled}
    />
  );
}

function MainListenerControlRequestRunner({
  onHandled,
  request,
}: {
  onHandled: (requestId: string) => void;
  request: ListenerControlRequest;
}) {
  const startListening = useStartListeningWithBatchOverride(request.sessionId);
  const stop = useListener((state) => state.stop);
  const startListeningRef = useRef(startListening);
  startListeningRef.current = startListening;
  const stopRef = useRef(stop);
  stopRef.current = stop;

  // Keyed on the request only: guard state is read imperatively at arrival
  // time, so the effect never re-runs (and never advances the queue) while
  // the requested start is still in flight.
  useEffect(() => {
    let cancelled = false;

    const run = async () => {
      const live = listenerStore.getState().live;
      const isStartingOrActive = live.loading || live.status === "active";

      if (request.action === "start") {
        // Non-main windows cannot see this window's state, so they can send
        // duplicate start requests for a session that is already starting.
        if (live.sessionId !== request.sessionId || !isStartingOrActive) {
          await startListeningRef.current();
        }
      } else if (live.sessionId === request.sessionId) {
        stopRef.current();
      }

      if (!cancelled) {
        onHandled(request.requestId);
      }
    };

    void run();

    return () => {
      cancelled = true;
    };
  }, [onHandled, request.action, request.requestId, request.sessionId]);

  return null;
}
