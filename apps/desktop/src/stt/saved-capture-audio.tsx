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

const DISMISSED_SAVED_CAPTURE_AUDIO_KEY =
  "anarlog:dismissed-saved-capture-audio-prompts";
const MAX_DISMISSED_SAVED_CAPTURE_AUDIO_PROMPTS = 128;

function useSavedCaptureAudioAt(sessionId: string) {
  const { data } = useLiveQuery<{ saved_at: string }, string | null>({
    sql: `SELECT updated_at AS saved_at FROM app_settings
      WHERE id = ? AND EXISTS (SELECT 1 FROM app_settings WHERE id = ?)
      LIMIT 1`,
    params: [
      `${CAPTURE_AUDIO_SAVED_SETTING_PREFIX}${sessionId}`,
      `${CAPTURE_LIFECYCLE_SETTING_PREFIX}${sessionId}`,
    ],
    mapRows: (rows) => rows[0]?.saved_at ?? null,
  });
  return data ?? null;
}

function readDismissedSavedCaptureAudioPrompts(): string[] {
  try {
    const stored: unknown = JSON.parse(
      localStorage.getItem(DISMISSED_SAVED_CAPTURE_AUDIO_KEY) ?? "[]",
    );
    return Array.isArray(stored)
      ? stored.filter((key): key is string => typeof key === "string")
      : [];
  } catch {
    return [];
  }
}

function rememberDismissedSavedCaptureAudioPrompt(promptKey: string) {
  try {
    const keys = readDismissedSavedCaptureAudioPrompts().filter(
      (key) => key !== promptKey,
    );
    keys.push(promptKey);
    localStorage.setItem(
      DISMISSED_SAVED_CAPTURE_AUDIO_KEY,
      JSON.stringify(keys.slice(-MAX_DISMISSED_SAVED_CAPTURE_AUDIO_PROMPTS)),
    );
  } catch {
    return;
  }
}

export function SavedCaptureAudioPrompt({ sessionId }: { sessionId: string }) {
  const savedAt = useSavedCaptureAudioAt(sessionId);
  const inactive = useListener(
    (state) => state.getSessionMode(sessionId) === "inactive",
  );
  const startListening = useStartListening(sessionId);

  useEffect(() => {
    if (savedAt === null || !inactive) return;
    // Keyed by the save time; starting a new capture clears it, so a later
    // unexpected stop prompts again.
    const promptKey = `${sessionId}:${savedAt}`;
    if (readDismissedSavedCaptureAudioPrompts().includes(promptKey)) return;
    const id = `capture-audio-saved-${sessionId}`;
    let rememberDismissal = true;
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
          rememberDismissal = false;
          void requestCaptureRecovery(sessionId).catch((error) => {
            console.error(
              "[listener] failed to request capture recovery",
              error,
            );
          });
        },
      },
      onDismiss: () => {
        if (rememberDismissal) {
          rememberDismissedSavedCaptureAudioPrompt(promptKey);
        }
      },
    });
    return () => toast.dismiss(id);
  }, [inactive, savedAt, sessionId, startListening]);

  return null;
}
