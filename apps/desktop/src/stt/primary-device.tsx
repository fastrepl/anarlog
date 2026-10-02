import type { SessionEvent } from "@anlg/store";
import { toast } from "@anlg/ui/components/ui/toast";

import { supabase } from "~/auth/client";
import { getDeviceIdentity } from "~/auth/cloudsync-credentials";
import {
  type MeetingDevice,
  type MeetingDeviceIntent,
  requestMeetingDevices,
} from "~/auth/sync-devices";
import { listenerStore } from "~/store/zustand/listener/instance";

export const PRIMARY_DEVICE_HEARTBEAT_MS = 10_000;
const REQUEST_TIMEOUT_MS = 5_000;

export type PrimaryDeviceDecision = "alone" | "primary" | "yield" | "ask";

export function decidePrimaryDevice(
  devices: MeetingDevice[],
  fingerprint: string,
): PrimaryDeviceDecision {
  const primary = devices.find((device) => device.primary);
  if (primary) {
    return primary.deviceFingerprint === fingerprint ? "primary" : "yield";
  }
  return devices.some((device) => device.deviceFingerprint !== fingerprint)
    ? "ask"
    : "alone";
}

export async function meetingKeyForEvent(
  event: Pick<SessionEvent, "tracking_id" | "started_at">,
): Promise<string | null> {
  if (!event.tracking_id || !event.started_at) {
    return null;
  }
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(`${event.tracking_id}\n${event.started_at}`),
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

function otherDeviceName(devices: MeetingDevice[], fingerprint: string) {
  const other = devices.find(
    (device) => device.deviceFingerprint !== fingerprint,
  );
  return other?.deviceName || "your other device";
}

const yieldedSessionIds = new Set<string>();
const coordinatedSessionIds = new Set<string>();
const pendingReleases = new Map<string, Promise<unknown>>();

// True once for a capture that stopped because another device is recording
// the same meeting; its local copy should be discarded.
export function consumePrimaryDeviceYield(sessionId: string) {
  return yieldedSessionIds.delete(sessionId);
}

async function sendHeartbeat(
  meetingKey: string,
  intent: MeetingDeviceIntent,
): Promise<{ devices: MeetingDevice[]; fingerprint: string } | null> {
  if (!supabase) {
    return null;
  }
  const { data } = await supabase.auth.getSession();
  const session = data.session;
  if (!session || session.user.is_anonymous === true) {
    return null;
  }
  const { fingerprint } = await getDeviceIdentity();
  if (!fingerprint) {
    return null;
  }
  const devices = await requestMeetingDevices({
    accessToken: session.access_token,
    fingerprint,
    meetingKey,
    intent,
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  return devices ? { devices, fingerprint } : null;
}

function isRecording(sessionId: string) {
  const { live } = listenerStore.getState();
  return live.sessionId === sessionId && live.status === "active";
}

// Coordinates with the user's other online devices so only one of them keeps
// recording a calendar meeting. Automatic starts announce themselves and ask
// the user which device they're joining from; the device the user answers or
// interacts with becomes primary and the others stop and discard their copy.
// Manual starts claim the meeting immediately. Without an account, Pro, or a
// network connection every device keeps recording.
export function startPrimaryDeviceCoordination({
  sessionId,
  event,
  automatic,
}: {
  sessionId: string;
  event: Pick<SessionEvent, "tracking_id" | "started_at"> | null;
  automatic: boolean;
}) {
  if (!event || coordinatedSessionIds.has(sessionId)) {
    return;
  }
  coordinatedSessionIds.add(sessionId);

  const toastId = `primary-device:${sessionId}`;
  let intent: MeetingDeviceIntent = automatic ? "present" : "claim";
  let stopped = false;
  let prompted = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let meetingKey: string | null = null;

  const onInteraction = (event: Event) => {
    if (
      event.target instanceof Element &&
      event.target
        .closest("[data-app-toast]")
        ?.querySelector("[data-primary-device-prompt]")
    ) {
      return;
    }
    claim();
  };
  const listenForInteraction = () => {
    window.addEventListener("pointerdown", onInteraction, true);
    window.addEventListener("keydown", onInteraction, true);
  };
  const stopListeningForInteraction = () => {
    window.removeEventListener("pointerdown", onInteraction, true);
    window.removeEventListener("keydown", onInteraction, true);
  };

  let inflight: Promise<unknown> = Promise.resolve();

  const finish = () => {
    if (stopped) return;
    stopped = true;
    clearTimeout(timer);
    stopListeningForInteraction();
    unsubscribe();
    toast.dismiss(toastId);
    coordinatedSessionIds.delete(sessionId);
    const key = meetingKey;
    if (key) {
      const release = inflight
        .catch(() => {})
        .then(() => sendHeartbeat(key, "release"))
        .catch(() => {});
      pendingReleases.set(sessionId, release);
      void release.then(() => {
        if (pendingReleases.get(sessionId) === release) {
          pendingReleases.delete(sessionId);
        }
      });
    }
  };

  const yieldTo = (name: string) => {
    if (!isRecording(sessionId)) {
      finish();
      return;
    }
    yieldedSessionIds.add(sessionId);
    finish();
    listenerStore.getState().stop();
    toast.info(`Recording on ${name}`, {
      id: `${toastId}:yielded`,
      description:
        "This device stopped recording the meeting and discarded its copy.",
    });
  };

  const beat = async () => {
    timer = undefined;
    if (stopped || !meetingKey) return;
    const sentIntent = intent;
    let result: Awaited<ReturnType<typeof sendHeartbeat>>;
    try {
      const request = sendHeartbeat(meetingKey, sentIntent);
      inflight = request;
      result = await request;
    } catch (error) {
      console.warn("[listener] meeting device heartbeat failed", error);
      if (!stopped) timer = setTimeout(beat, PRIMARY_DEVICE_HEARTBEAT_MS);
      return;
    }
    if (stopped) return;
    if (!result) {
      meetingKey = null;
      finish();
      return;
    }
    if (intent === "claim" && sentIntent !== "claim") {
      void beat();
      return;
    }
    if (sentIntent === "claim") {
      intent = "present";
    }
    const { devices, fingerprint } = result;
    const decision = decidePrimaryDevice(devices, fingerprint);
    if (decision === "yield") {
      yieldTo(
        devices.find((device) => device.primary)?.deviceName ||
          "your other device",
      );
      return;
    }
    if (decision === "ask" && !prompted) {
      prompted = true;
      showPrompt(otherDeviceName(devices, fingerprint));
      listenForInteraction();
    } else if (decision !== "ask" && prompted) {
      prompted = false;
      toast.dismiss(toastId);
      stopListeningForInteraction();
    }
    timer = setTimeout(beat, PRIMARY_DEVICE_HEARTBEAT_MS);
  };

  const claim = () => {
    if (stopped || intent === "claim") return;
    intent = "claim";
    prompted = false;
    toast.dismiss(toastId);
    stopListeningForInteraction();
    if (timer !== undefined) {
      clearTimeout(timer);
      void beat();
    }
  };

  const showPrompt = (otherName: string) => {
    toast("Is this your primary device?", {
      id: toastId,
      duration: Infinity,
      dismissible: false,
      description: (
        <div data-primary-device-prompt className="flex gap-3">
          <button
            type="button"
            onClick={claim}
            className="text-foreground font-medium underline-offset-2 hover:underline"
          >
            Yes
          </button>
          <button
            type="button"
            onClick={() => yieldTo(otherName)}
            className="text-foreground font-medium underline-offset-2 hover:underline"
          >
            No
          </button>
        </div>
      ),
    });
  };

  const unsubscribe = listenerStore.subscribe(() => {
    if (!isRecording(sessionId)) finish();
  });

  const previousRelease = pendingReleases.get(sessionId) ?? Promise.resolve();
  void Promise.all([meetingKeyForEvent(event), previousRelease]).then(
    ([key]) => {
      if (stopped) return;
      if (!key) {
        finish();
        return;
      }
      meetingKey = key;
      void beat();
    },
    () => finish(),
  );
}
