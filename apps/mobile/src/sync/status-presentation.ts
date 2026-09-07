import type { MobileSyncPhase, MobileSyncSnapshot } from "./controller";

const phaseCopy: Record<
  MobileSyncPhase,
  { title: string; description: string }
> = {
  inactive: {
    title: "Cloud sync is off",
    description: "Sign in with Anarlog Pro to sync this device.",
  },
  starting: {
    title: "Connecting this device",
    description: "Preparing your encrypted workspace…",
  },
  approval_pending: {
    title: "Approve this device on desktop",
    description:
      "Open Anarlog on a synced computer signed in to the same account. In Settings → Sync → Devices, approve this phone. Sync will start automatically. Your local notes are safe while you wait.",
  },
  ready: {
    title: "Cloud sync is on",
    description: "Your notes sync end-to-end encrypted across your devices.",
  },
  error: {
    title: "Cloud sync needs attention",
    description: "Your notes are safe on this device. Try connecting again.",
  },
  device_limit: {
    title: "Device limit reached",
    description:
      "Pro includes three synced devices. Team includes five per member. Remove a device to free a slot.",
  },
  identity_mismatch: {
    title: "Reconnect this device",
    description:
      "Remove this device in Device Management, then sign in here again.",
  },
  not_entitled: {
    title: "Anarlog Pro required",
    description: "Refresh your plan to continue syncing this device.",
  },
  reauth_required: {
    title: "Sign in again",
    description: "Your session expired before cloud sync could connect.",
  },
  account_mismatch: {
    title: "This device belongs to another account",
    description:
      "Your local notes were created by a different account, so sync stays off to protect both workspaces.",
  },
};

function relativeSyncTime(
  lastSyncAtMs: number | null,
  nowMs: number,
): string | null {
  if (!lastSyncAtMs) return null;
  const elapsedMinutes = Math.max(
    0,
    Math.floor((nowMs - lastSyncAtMs) / 60_000),
  );
  if (elapsedMinutes < 1) return "Synced just now";
  if (elapsedMinutes < 60) return `Synced ${elapsedMinutes}m ago`;
  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) return `Synced ${elapsedHours}h ago`;
  return `Synced ${Math.floor(elapsedHours / 24)}d ago`;
}

export function syncStatusPresentation(
  snapshot: MobileSyncSnapshot,
  nowMs = Date.now(),
): {
  title: string;
  description: string;
  detail: string | null;
  healthy: boolean;
  pending: boolean;
  retrying: boolean;
} {
  const retrying =
    snapshot.phase === "ready" &&
    snapshot.running &&
    (snapshot.consecutiveFailures > 0 || snapshot.errorMessage !== null);
  const paused = snapshot.phase === "ready" && !snapshot.running;
  const awaitingFirstSync =
    snapshot.phase === "ready" &&
    snapshot.running &&
    snapshot.lastSyncAtMs === null &&
    !retrying;
  const copy = retrying
    ? {
        title: "Cloud sync is retrying",
        description:
          "Your notes are safe on this device. Anarlog will keep trying in the background.",
      }
    : paused
      ? {
          title: "Cloud sync is paused",
          description:
            "Your notes are safe on this device. Sync this device again when you are online.",
        }
      : awaitingFirstSync
        ? {
            title: "Finishing cloud sync",
            description:
              "This device is connected. Waiting for its first encrypted sync to finish.",
          }
        : phaseCopy[snapshot.phase];

  const detail =
    snapshot.phase !== "ready"
      ? snapshot.errorMessage
      : snapshot.syncingNow
        ? "Syncing now…"
        : paused
          ? (snapshot.errorMessage ?? "Sync is paused.")
          : retrying
            ? (snapshot.errorMessage ?? "Sync will retry automatically.")
            : snapshot.hasUnsentChanges
              ? "Changes waiting to sync"
              : awaitingFirstSync
                ? "Waiting for first sync"
                : relativeSyncTime(snapshot.lastSyncAtMs, nowMs);

  const pending =
    snapshot.phase === "approval_pending" ||
    (snapshot.phase === "ready" &&
      !retrying &&
      (snapshot.syncingNow ||
        snapshot.hasUnsentChanges === true ||
        awaitingFirstSync));

  return {
    ...copy,
    detail,
    healthy:
      snapshot.phase === "ready" &&
      snapshot.running &&
      snapshot.lastSyncAtMs !== null &&
      !retrying &&
      !pending,
    pending,
    retrying,
  };
}
