import { useCallback, useMemo } from "react";

import { useMainStoreInternal } from "~/session/hooks/internal";
import * as main from "~/store/tinybase/store/main";

export function useSessionTagMappingIds(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.tagSessionsBySession,
    sessionId,
    main.STORE_ID,
  );
}

export function useTagMappingCell(
  mappingId: string,
  field: "tag_id" | "session_id",
): string {
  const v = main.UI.useCell(
    "mapping_tag_session",
    mappingId,
    field,
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

export function useTagName(tagId: string): string {
  const v = main.UI.useCell("tags", tagId, "name", main.STORE_ID);
  return (v as string | undefined) ?? "";
}

export function useSessionTagMutations(): {
  createTag: (args: { id: string; name: string }) => void;
  addTagToSession: (args: { tagId: string; sessionId: string }) => string;
  deleteTagMapping: (mappingId: string) => void;
} {
  const store = useMainStoreInternal();

  const createTag = useCallback(
    ({ id, name }: { id: string; name: string }) => {
      if (!store) return;
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      store.setRow("tags", id, { user_id: userId, name });
    },
    [store],
  );

  const addTagToSession = useCallback(
    ({ tagId, sessionId }: { tagId: string; sessionId: string }) => {
      if (!store) return "";
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      const mappingId = crypto.randomUUID();
      store.setRow("mapping_tag_session", mappingId, {
        user_id: userId,
        tag_id: tagId,
        session_id: sessionId,
      });
      return mappingId;
    },
    [store],
  );

  const deleteTagMapping = useCallback(
    (mappingId: string) => {
      if (!store) return;
      store.delRow("mapping_tag_session", mappingId);
    },
    [store],
  );

  return { createTag, addTagToSession, deleteTagMapping };
}

export function useSessionTagNameMap(sessionId: string): Map<string, string> {
  const mappingIds = useSessionTagMappingIds(sessionId);
  const mappingTable = main.UI.useTable("mapping_tag_session", main.STORE_ID);
  const tagsTable = main.UI.useTable("tags", main.STORE_ID);

  return useMemo(() => {
    const byName = new Map<string, string>();

    for (const mappingId of mappingIds) {
      const tagId = mappingTable[mappingId]?.tag_id;
      if (typeof tagId !== "string" || !tagId) continue;

      const tagName = tagsTable[tagId]?.name;
      if (typeof tagName !== "string") continue;

      byName.set(tagName.toLowerCase(), tagId);
    }

    return byName;
  }, [mappingIds, mappingTable, tagsTable]);
}

export function useAddSessionTag(sessionId: string): (name: string) => void {
  const existingTagIdsByName = useSessionTagNameMap(sessionId);
  const store = useMainStoreInternal();

  return useCallback(
    (name: string) => {
      if (!store) return;

      const userId = store.getValue("user_id") as string | undefined;
      if (!userId) return;

      const normalized = name.toLowerCase();
      const existingTagId = existingTagIdsByName.get(normalized);

      let tagId = existingTagId;
      if (!tagId) {
        let foundTagId: string | null = null;
        store.forEachRow("tags", (rowId, _forEachCell) => {
          if (foundTagId) return;

          const tagName = store.getCell("tags", rowId, "name");
          if (
            typeof tagName === "string" &&
            tagName.toLowerCase() === normalized
          ) {
            foundTagId = rowId;
          }
        });

        tagId = foundTagId ?? crypto.randomUUID();
        if (!foundTagId) {
          store.setRow("tags", tagId, {
            user_id: userId,
            name,
          });
        }
      }

      let hasMapping = false;
      store.forEachRow("mapping_tag_session", (mappingId, _forEachCell) => {
        if (hasMapping) return;
        const existingSessionId = store.getCell(
          "mapping_tag_session",
          mappingId,
          "session_id",
        );
        const existingMappedTagId = store.getCell(
          "mapping_tag_session",
          mappingId,
          "tag_id",
        );
        hasMapping =
          existingSessionId === sessionId && existingMappedTagId === tagId;
      });

      if (hasMapping) return;

      const mappingId = crypto.randomUUID();
      store.setRow("mapping_tag_session", mappingId, {
        user_id: userId,
        tag_id: tagId,
        session_id: sessionId,
      });
    },
    [existingTagIdsByName, sessionId, store],
  );
}
