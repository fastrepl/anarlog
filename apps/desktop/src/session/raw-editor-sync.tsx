import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import * as main from "~/store/tinybase/store/main";

const RAW_EDITOR_SYNC_EVENT = "anarlog-raw-editor-sync";

type RawEditorSyncPayload = {
  sessionId: string;
  rawMd: string;
  sourceId: string;
};
type RawEditorStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

export function RawEditorSyncBridge() {
  const store = main.UI.useStore(main.STORE_ID);

  if (!store) {
    return null;
  }

  return <MountedRawEditorSyncBridge store={store} />;
}

export function emitRawEditorSync(payload: RawEditorSyncPayload) {
  void emitTo("main", RAW_EDITOR_SYNC_EVENT, payload).catch((error) => {
    console.error("Failed to sync raw editor content to main window:", error);
  });
}

function MountedRawEditorSyncBridge({ store }: { store: RawEditorStore }) {
  useMountEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    void listen<RawEditorSyncPayload>(RAW_EDITOR_SYNC_EVENT, ({ payload }) => {
      if (!isRawEditorSyncPayload(payload)) {
        return;
      }

      if (
        store.getCell("sessions", payload.sessionId, "raw_md") === payload.rawMd
      ) {
        return;
      }

      store.setCell("sessions", payload.sessionId, "raw_md", payload.rawMd);
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten();
      } else {
        unlisten = nextUnlisten;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  return null;
}

function isRawEditorSyncPayload(
  payload: unknown,
): payload is RawEditorSyncPayload {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Partial<RawEditorSyncPayload>;
  return (
    typeof candidate.sessionId === "string" &&
    typeof candidate.rawMd === "string" &&
    typeof candidate.sourceId === "string"
  );
}
