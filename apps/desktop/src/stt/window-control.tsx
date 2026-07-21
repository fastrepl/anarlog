import { emitTo, listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { useListener } from "./contexts";
import { useStartListening } from "./useStartListening";

import { listenerStore } from "~/store/zustand/listener/instance";

const LISTENER_CONTROL_EVENT = "hypr:listener-control";

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
  await emitTo("main", LISTENER_CONTROL_EVENT, {
    action,
    requestId: crypto.randomUUID(),
    sessionId,
  } satisfies ListenerControlRequest);
}

export function MainListenerControlBridge() {
  const [requests, setRequests] = useState<ListenerControlRequest[]>([]);

  useEffect(() => {
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
  }, []);

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
  const startListening = useStartListening(request.sessionId);
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
