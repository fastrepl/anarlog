import { useRouter } from "expo-router";
import { useRef } from "react";

import {
  beginMobileCapture,
  endMobileCapture,
  getMobileCaptureActive,
  stopMobileCapture,
} from "@/audio/capture-lifecycle";
import { createSession } from "@/data/session";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";
import {
  consumePendingQuickAction,
  subscribeQuickActions,
} from "@/quick-actions";

export function QuickActionLifecycle({
  accountUserId,
}: {
  accountUserId: string | null;
}) {
  const router = useRouter();
  const busyRef = useRef(false);
  const queuedToggleRef = useRef(false);

  const runToggle = async () => {
    busyRef.current = true;
    try {
      if (getMobileCaptureActive()) {
        await stopMobileCapture();
        return;
      }

      const sessionId = await createSession({
        entryPoint: "start_listening",
        ownerUserId: accountUserId ?? undefined,
      });
      beginMobileCapture(sessionId);
      try {
        router.push(`/note/${sessionId}?listen=1`);
      } catch (error) {
        endMobileCapture(sessionId);
        throw error;
      }
    } catch (error) {
      captureOperationalError(error, {
        operation: "quick_action_toggle_listening",
      });
    } finally {
      busyRef.current = false;
      if (queuedToggleRef.current) {
        queuedToggleRef.current = false;
        void runToggle();
      }
    }
  };

  const handleAction = () => {
    const action = consumePendingQuickAction();
    if (action !== "toggle_listening") return;
    if (busyRef.current) {
      queuedToggleRef.current = !queuedToggleRef.current;
      return;
    }
    void runToggle();
  };

  useMountEffect(() => {
    const unsubscribe = subscribeQuickActions(handleAction);
    handleAction();
    return unsubscribe;
  });

  return null;
}
