import { createJsonFilePersister } from "~/store/tinybase/persister/factories";
import type { Store } from "~/store/tinybase/store/main";

export function createChatShortcutPersister(store: Store) {
  return createJsonFilePersister(store, {
    tableName: "chat_shortcuts",
    filename: "chat_shortcuts.json",
    label: "ChatShortcutPersister",
  });
}
