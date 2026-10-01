import { useEffect } from "react";

import { toast } from "@anlg/ui/components/ui/toast";

import {
  CAPTURE_AUDIO_SAVED_SETTING_PREFIX,
  CAPTURE_LIFECYCLE_SETTING_PREFIX,
} from "./capture-lifecycle-storage";
import { requestCaptureRecovery } from "./capture-recovery-requests";
import { useListener } from "./contexts";
import { useStartListening } from "./useStartListening";
import {
  isMainWebviewWindow,
  requestMainListenerControl,
} from "./window-control";

import { useLiveQuery } from "~/db";

function useSavedCaptureAudio(sessionId: string) {
  const { data } = useLiveQuery<{ saved: number }, boolean>({
    sql: `SELECT 1 AS saved FROM app_settings
      WHERE id = ? AND EXISTS (SELECT 1 FROM app_settings WHERE id = ?)
      LIMIT 1`,
    params: [
      `${CAPTURE_AUDIO_SAVED_SETTING_PREFIX}${sessionId}`,
      `${CAPTURE_LIFECYCLE_SETTING_PREFIX}${sessionId}`,
    ],
    mapRows: (rows) => rows.length > 0,
  });
  return data === true;
}

export function SavedCaptureAudioPrompt({ sessionId }: { sessionId: string }) {
  const saved = useSavedCaptureAudio(sessionId);
  const inactive = useListener(
    (state) => state.getSessionMode(sessionId) === "inactive",
  );
  const startListening = useStartListening(sessionId);

  useEffect(() => {
    if (!saved || !inactive) return;
    const id = `capture-audio-saved-${sessionId}`;
    const resumeListening = () => {
      const start = isMainWebviewWindow()
        ? startListening()
        : requestMainListenerControl("start", sessionId);
      void start.catch((error) => {
        console.error("[listener] failed to resume listening", error);
      });
    };
    toast.warning("Anarlog saved this meeting's audio", {
      id,
      duration: Infinity,
      description: (
        <div className="space-y-2">
          <p>
            Recording stopped unexpectedly, so the audio was kept on purpose.
            Create the meeting note to transcribe and summarize it, or resume
            listening. The audio is deleted once it is transcribed.
          </p>
          <button
            type="button"
            onClick={resumeListening}
            className="text-foreground font-medium underline-offset-2 hover:underline"
          >
            Resume listening
          </button>
        </div>
      ),
      action: {
        label: "Create meeting note",
        onClick: () => {
          void requestCaptureRecovery(sessionId).catch((error) => {
            console.error(
              "[listener] failed to request capture recovery",
              error,
            );
          });
        },
      },
    });
    return () => toast.dismiss(id);
  }, [inactive, saved, sessionId, startListening]);

  return null;
}
