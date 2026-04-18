import { useCallback, useMemo } from "react";

import type { SessionEvent } from "@hypr/store";

import {
  useMainIndexesInternal,
  useMainStoreInternal,
} from "~/session/hooks/internal";
import { getSessionEvent } from "~/session/utils";
import { useSessionTabLifecycle as useSharedSessionTabLifecycle } from "~/shared/desktop-tab-lifecycle";
import * as main from "~/store/tinybase/store/main";

export type SessionListItem = {
  id: string;
  title: string;
  createdAt: string;
};

export type SessionFolderTree = {
  topLevel: string[];
  byParent: Record<string, string[]>;
};

type SessionStringField =
  | "title"
  | "raw_md"
  | "created_at"
  | "event_json"
  | "folder_id"
  | "user_id";

export function useSessionTabLifecycle(args: {
  onEmpty?: (() => void) | null;
  onZeroTabs?: (() => void) | null;
}) {
  const store = useMainStoreInternal();
  const indexes = useMainIndexesInternal();

  useSharedSessionTabLifecycle({
    store,
    indexes,
    onEmpty: args.onEmpty,
    onZeroTabs: args.onZeroTabs,
  });
}

export function useSessionCell(
  sessionId: string,
  field: SessionStringField,
): string {
  return useSessionCellOrEmpty(sessionId, field);
}

export function useSessionCellOrEmpty(
  sessionId: string,
  field: SessionStringField,
): string {
  const v = main.UI.useCell("sessions", sessionId, field, main.STORE_ID);
  return (v as string | undefined) ?? "";
}

export function useSessionCellOptional(
  sessionId: string,
  field: SessionStringField,
): string | undefined {
  const v = main.UI.useCell("sessions", sessionId, field, main.STORE_ID);
  return v as string | undefined;
}

// Some header.tsx paths read fields that aren't in the schema (returns
// undefined). Kept as a loose-typed helper so header migration can compile.
export function useSessionUntypedCell(
  sessionId: string,
  field: string,
): string | undefined {
  const cell = main.UI.useCell(
    "sessions",
    sessionId,
    field as "title",
    main.STORE_ID,
  );
  return cell as string | undefined;
}

export function useAllSessionIds(): string[] {
  return main.UI.useRowIds("sessions", main.STORE_ID);
}

export function useSession(sessionId: string) {
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  const folderId = main.UI.useCell(
    "sessions",
    sessionId,
    "folder_id",
    main.STORE_ID,
  );

  const event = useMemo(
    () => getSessionEvent({ event_json: eventJson }),
    [eventJson],
  );

  return useMemo(
    () => ({ title, rawMd, createdAt, event, folderId }),
    [title, rawMd, createdAt, event, folderId],
  );
}

export function useOpenNoteSessions(): SessionListItem[] {
  const sessionIds = useAllSessionIds();
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useMemo(() => {
    return sessionIds
      .map((id) => {
        const session = sessionsTable[id];
        return {
          id,
          title:
            typeof session?.title === "string" && session.title
              ? session.title
              : "Untitled",
          createdAt:
            typeof session?.created_at === "string" ? session.created_at : "",
        } satisfies SessionListItem;
      })
      .sort((a, b) => {
        if (!a.createdAt || !b.createdAt) return 0;
        return (
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        );
      });
  }, [sessionIds, sessionsTable]);
}

export function useSessionFolderTree(): SessionFolderTree {
  const sessionIds = useAllSessionIds();
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useMemo(() => {
    const allFolders = new Set<string>();

    for (const id of sessionIds) {
      const folderId = sessionsTable[id]?.folder_id;
      if (typeof folderId !== "string" || !folderId) continue;

      const parts = folderId.split("/");
      for (let i = 1; i <= parts.length; i++) {
        allFolders.add(parts.slice(0, i).join("/"));
      }
    }

    const topLevel: string[] = [];
    const byParent: Record<string, string[]> = {};

    for (const folder of allFolders) {
      const parts = folder.split("/");
      if (parts.length === 1) {
        topLevel.push(folder);
      } else {
        const parent = parts.slice(0, -1).join("/");
        byParent[parent] = byParent[parent] || [];
        byParent[parent].push(folder);
      }
    }

    return {
      topLevel: topLevel.sort(),
      byParent: Object.fromEntries(
        Object.entries(byParent).map(([key, value]) => [key, value.sort()]),
      ),
    };
  }, [sessionIds, sessionsTable]);
}

export function useSessionEvent(sessionId: string): SessionEvent | null {
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  return useMemo(() => getSessionEvent({ event_json: eventJson }), [eventJson]);
}

export function useSessionIdsInFolder(folderId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.sessionsByFolder,
    folderId,
    main.STORE_ID,
  );
}

export function useUpdateSessionCell(
  sessionId: string,
  field: SessionStringField,
): (value: string) => void {
  const store = useMainStoreInternal();
  return useCallback(
    (value) => {
      if (!store) return;
      store.setPartialRow("sessions", sessionId, {
        [field]: value,
      } as Record<string, string>);
    },
    [store, sessionId, field],
  );
}

export function useUpdateSessionRawMd(
  sessionId: string,
): (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => void {
  return main.UI.useSetPartialRowCallback(
    "sessions",
    sessionId,
    (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => ({
      raw_md: e.target.value,
    }),
    [],
    main.STORE_ID,
  );
}

export function useSetSessionCreatedAt(
  sessionId: string,
): (created_at: string) => void {
  return main.UI.useSetCellCallback(
    "sessions",
    sessionId,
    "created_at",
    (created_at: string) => created_at,
    [],
    main.STORE_ID,
  );
}
