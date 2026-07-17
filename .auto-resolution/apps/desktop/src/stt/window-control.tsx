import { emitTo, listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";

import { useListener } from "./contexts";
import { useStartListening } from "./useStartListening";

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
  const activeSessionId = useListener((state) => state.live.sessionId);

  useEffect(() => {
    let cancelled = false;

    const run = async () => {
      if (request.action === "start") {
        await startListening();
      } else if (activeSessionId === request.sessionId) {
        stop();
      }

      if (!cancelled) {
        onHandled(request.requestId);
      }
    };

    void run();

    return () => {
      cancelled = true;
    };
  }, [
    activeSessionId,
    onHandled,
    request.action,
    request.requestId,
    request.sessionId,
    startListening,
    stop,
  ]);

  return null;
}
