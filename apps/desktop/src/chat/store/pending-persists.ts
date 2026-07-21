// Outbound chat persists that are still in flight (including retries).
// ChatSession's reconciliation replaces in-memory messages with the SQLite
// set once a stream settles; while a send's write is pending that set is
// legitimately behind, so reconciliation must hold off or it wipes the turn
// the user just sent.
const pendingByGroup = new Map<string, number>();

export function beginPendingChatPersist(chatGroupId: string) {
  pendingByGroup.set(chatGroupId, (pendingByGroup.get(chatGroupId) ?? 0) + 1);
}

export function endPendingChatPersist(chatGroupId: string) {
  const count = pendingByGroup.get(chatGroupId) ?? 0;
  if (count <= 1) {
    pendingByGroup.delete(chatGroupId);
  } else {
    pendingByGroup.set(chatGroupId, count - 1);
  }
}

export function hasPendingChatPersist(chatGroupId: string) {
  return pendingByGroup.has(chatGroupId);
}
