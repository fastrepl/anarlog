// In-flight soft-delete writes by session id. The undo toast appears before
// the tombstone commits, so a restore must wait for the write to settle —
// otherwise "session still alive" can mean "delete not committed yet" and the
// in-flight delete lands after the undo, vanishing the note with no toast.
const pending = new Map<string, Promise<unknown>>();

export function trackPendingSoftDelete(
  sessionId: string,
  write: Promise<unknown>,
) {
  const entry = write
    .catch(() => undefined)
    .finally(() => {
      if (pending.get(sessionId) === entry) {
        pending.delete(sessionId);
      }
    });
  pending.set(sessionId, entry);
}

export function waitForPendingSoftDelete(sessionId: string): Promise<unknown> {
  return pending.get(sessionId) ?? Promise.resolve();
}
