import { DEVICE_AUTH_REASON } from "./auth";
import { useAppLock } from "./store";

import { updateSession } from "~/session/queries";

export async function setSessionLocked(
  sessionId: string,
  locked: boolean,
): Promise<boolean> {
  const reason = locked
    ? DEVICE_AUTH_REASON.lockNote
    : DEVICE_AUTH_REASON.unlockNote;
  const ok = await useAppLock.getState().authenticate(reason);
  if (!ok) return false;
  await updateSession(sessionId, { locked });
  if (locked) {
    useAppLock.getState().concealNote(sessionId);
  } else {
    useAppLock.getState().markNoteRevealed(sessionId);
  }
  return true;
}

export async function revealLockedNote(sessionId: string): Promise<boolean> {
  return useAppLock
    .getState()
    .revealNote(sessionId, DEVICE_AUTH_REASON.openApp);
}
