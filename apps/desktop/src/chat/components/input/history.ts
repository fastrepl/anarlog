import type { JSONContent } from "@anlg/editor/chat";

const MAX_ENTRIES = 50;

// Newest first, shared across chats so a prompt can be recalled in a new chat.
const entries: JSONContent[] = [];

export function pushSentMessage(json: JSONContent | undefined) {
  if (!json) {
    return;
  }

  const serialized = JSON.stringify(json);
  if (entries.length > 0 && JSON.stringify(entries[0]) === serialized) {
    return;
  }

  entries.unshift(json);
  if (entries.length > MAX_ENTRIES) {
    entries.length = MAX_ENTRIES;
  }
}

export function sentMessageCount() {
  return entries.length;
}

export function sentMessageAt(index: number) {
  return entries[index];
}

export function clearSentMessages() {
  entries.length = 0;
}
