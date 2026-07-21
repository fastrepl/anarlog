// Outbound chat persists that are still in flight (including retries).
// ChatSession's reconciliation replaces in-memory messages with the SQLite
// set once a stream settles; while a send's write is pending that set is
// legitimately behind, so reconciliation must hold off or it wipes the turn
// the user just sent. onFinish also awaits settlement before persisting the
// assistant, so a failed user persist can be repaired first.
const pendingByGroup = new Map<string, Set<Promise<unknown>>>();

// Group ids whose chat_groups row creation terminally failed. Persisting
// messages into them would create permanent orphans that never appear in
// history. Ids are never reused, so entries can live for the session.
const failedGroupCreates = new Set<string>();

export function trackPendingChatPersist(
  chatGroupId: string,
  write: Promise<unknown>,
) {
  const settled: Promise<unknown> = write
    .catch(() => undefined)
    .finally(() => {
      const writes = pendingByGroup.get(chatGroupId);
      if (!writes) return;
      writes.delete(settled);
      if (writes.size === 0) {
        pendingByGroup.delete(chatGroupId);
      }
    });
  const writes = pendingByGroup.get(chatGroupId) ?? new Set();
  writes.add(settled);
  pendingByGroup.set(chatGroupId, writes);
}

export function hasPendingChatPersist(chatGroupId: string) {
  return pendingByGroup.has(chatGroupId);
}

export async function waitForPendingChatPersists(chatGroupId: string) {
  // New writes can be tracked while earlier ones are awaited; a few rounds
  // cover realistic pile-ups without risking an unbounded loop.
  for (let round = 0; round < 5; round += 1) {
    const writes = pendingByGroup.get(chatGroupId);
    if (!writes || writes.size === 0) return;
    await Promise.all([...writes]);
  }
}

export function markFailedChatGroupCreate(chatGroupId: string) {
  failedGroupCreates.add(chatGroupId);
}

export function isFailedChatGroupCreate(chatGroupId: string) {
  return failedGroupCreates.has(chatGroupId);
}
