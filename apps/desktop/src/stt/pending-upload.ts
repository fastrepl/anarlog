type PendingUpload = { kind: "audio" | "transcript"; filePath: string };

const MAX_PENDING_UPLOADS = 64;
const PENDING_UPLOAD_TTL_MS = 30 * 60 * 1_000;

type PendingUploadEntry = {
  upload: PendingUpload;
  expiresAt: number;
};

const pending = new Map<string, PendingUploadEntry>();
const reservations = new Set<symbol>();
let expirationTimer: ReturnType<typeof setTimeout> | null = null;

function removeExpired(now: number): void {
  for (const [sessionId, entry] of pending) {
    if (entry.expiresAt <= now) {
      pending.delete(sessionId);
    }
  }
}

function scheduleExpiration(): void {
  if (expirationTimer) {
    clearTimeout(expirationTimer);
    expirationTimer = null;
  }

  let nextExpiration = Number.POSITIVE_INFINITY;
  for (const entry of pending.values()) {
    nextExpiration = Math.min(nextExpiration, entry.expiresAt);
  }
  if (!Number.isFinite(nextExpiration)) {
    return;
  }

  expirationTimer = setTimeout(
    () => {
      expirationTimer = null;
      removeExpired(Date.now());
      scheduleExpiration();
    },
    Math.max(0, nextExpiration - Date.now()),
  );
}

function hasCapacity(): boolean {
  removeExpired(Date.now());
  return pending.size + reservations.size < MAX_PENDING_UPLOADS;
}

export function reservePendingUpload(upload: PendingUpload): {
  commit: (sessionId: string) => boolean;
  cancel: () => void;
} | null {
  if (!hasCapacity()) {
    return null;
  }

  const token = Symbol();
  reservations.add(token);
  let active = true;

  return {
    commit(sessionId) {
      if (!active || !reservations.delete(token)) {
        return false;
      }
      active = false;
      pending.set(sessionId, {
        upload,
        expiresAt: Date.now() + PENDING_UPLOAD_TTL_MS,
      });
      scheduleExpiration();
      return true;
    },
    cancel() {
      if (!active) {
        return;
      }
      active = false;
      reservations.delete(token);
    },
  };
}

export function setPendingUpload(
  sessionId: string,
  upload: PendingUpload,
): boolean {
  const existing = pending.get(sessionId);
  if (!existing && !hasCapacity()) {
    return false;
  }

  pending.set(sessionId, {
    upload,
    expiresAt: Date.now() + PENDING_UPLOAD_TTL_MS,
  });
  scheduleExpiration();
  return true;
}

export function consumePendingUpload(sessionId: string): PendingUpload | null {
  const entry = pending.get(sessionId);
  if (entry && entry.expiresAt > Date.now()) {
    pending.delete(sessionId);
    scheduleExpiration();
    return entry.upload;
  }
  clearPendingUpload(sessionId);
  return null;
}

export function clearPendingUpload(sessionId: string): void {
  if (!pending.delete(sessionId)) {
    return;
  }
  scheduleExpiration();
}
