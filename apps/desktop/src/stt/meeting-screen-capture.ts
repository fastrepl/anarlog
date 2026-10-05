import { platform } from "@tauri-apps/plugin-os";

import { commands as detectCommands } from "@anlg/plugin-detect";
import { commands as permissionsCommands } from "@anlg/plugin-permissions";
import { toast } from "@anlg/ui/components/ui/toast";

import { getStoredSettingValues } from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { persistMeetingScreenCapture } from "~/stt/meeting-screen-records";

const MEETING_SCREEN_CAPTURE_INTERVAL_MS = 3_000;

export function startMeetingScreenCapture({
  sessionId,
  isEnabled,
}: {
  sessionId: string;
  isEnabled?: () => boolean | Promise<boolean>;
}) {
  let stopped = false;
  let resetSampler = true;
  let inFlight: Promise<void> | null = null;
  let timeout: ReturnType<typeof setTimeout> | null = null;
  let permissionWarned = false;
  const captureIsEnabled =
    isEnabled ??
    (async () =>
      resolveConfigValue(
        "capture_shared_screens",
        await getStoredSettingValues(),
      ));

  const captureOnce = async () => {
    try {
      if (!(await captureIsEnabled())) {
        resetSampler = true;
        return;
      }

      const permission =
        platform() === "macos"
          ? await permissionsCommands.checkPermission("screenRecording")
          : null;
      if (permission?.status === "ok" && permission.data === "denied") {
        if (!permissionWarned) {
          permissionWarned = true;
          toast.warning(
            "Shared screen capture needs Screen Recording permission in Settings",
            { id: "meeting-screen-capture-warning", duration: Infinity },
          );
        }
        return;
      }

      const result =
        await detectCommands.captureMeetingScreenShare(resetSampler);
      resetSampler = false;
      if (stopped) {
        return;
      }
      if (!(await captureIsEnabled()) || stopped) {
        resetSampler = true;
        return;
      }
      if (result.status === "error") {
        console.warn(
          "[listener] failed to capture shared screen",
          result.error,
        );
        return;
      }
      if (!result.data.jpeg) {
        return;
      }

      await persistMeetingScreenCapture({ sessionId, capture: result.data });
    } catch (error) {
      resetSampler = true;
      console.warn("[listener] failed to capture shared screen", error);
    }
  };

  const scheduleCapture = () => {
    if (stopped || timeout) {
      return;
    }
    timeout = setTimeout(() => {
      timeout = null;
      void capture();
    }, MEETING_SCREEN_CAPTURE_INTERVAL_MS);
  };

  const capture = () => {
    if (stopped || inFlight) {
      return;
    }
    const pendingCapture = captureOnce().finally(() => {
      if (inFlight === pendingCapture) {
        inFlight = null;
        scheduleCapture();
      }
    });
    inFlight = pendingCapture;
  };

  void capture();

  return async () => {
    stopped = true;
    if (timeout) {
      clearTimeout(timeout);
      timeout = null;
    }
    await inFlight;
  };
}
