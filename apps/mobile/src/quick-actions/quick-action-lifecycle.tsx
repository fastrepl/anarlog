import { useRouter } from "expo-router";
import { useRef } from "react";

import {
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

  const handleAction = async () => {
    if (busyRef.current) return;
    const action = consumePendingQuickAction();
    if (action !== "toggle_listening") return;

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
      router.push(`/note/${sessionId}?listen=1`);
    } catch (error) {
      captureOperationalError(error, {
        operation: "quick_action_toggle_listening",
      });
    } finally {
      busyRef.current = false;
    }
  };

  useMountEffect(() => {
    const unsubscribe = subscribeQuickActions(() => void handleAction());
    void handleAction();
    return unsubscribe;
  });

  return null;
}
