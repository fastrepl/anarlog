import { useCallback, useMemo, useRef, useState } from "react";

import { TagChip } from "./chip";

import {
  useMainStore,
  useSessionTagMappingIds,
  useSessionTagMutations,
} from "~/session/hooks/storage";

export function TagInput({ sessionId }: { sessionId: string }) {
  const [inputValue, setInputValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const { mappingIds, existingTagIdsByName } = useSessionTags(sessionId);
  const addTag = useAddTag(sessionId, existingTagIdsByName);
  const { deleteTagMapping } = useSessionTagMutations();

  const placeholder =
    mappingIds.length > 0 ? "Add another tag" : "Add tags to this note";

  const submitCurrentTag = useCallback(() => {
    const next = normalizeTag(inputValue);
    if (!next) {
      return;
    }

    addTag(next);
    setInputValue("");
  }, [addTag, inputValue]);

  return (
    <div
      className="flex min-h-[38px] w-full cursor-text flex-wrap items-center gap-2"
      onClick={() => inputRef.current?.focus()}
    >
      {mappingIds.map((mappingId) => (
        <TagChip key={mappingId} mappingId={mappingId} />
      ))}

      <input
        ref={inputRef}
        type="text"
        className="min-w-[100px] flex-1 bg-transparent text-sm outline-hidden placeholder:text-neutral-400"
        value={inputValue}
        placeholder={placeholder}
        onChange={(e) => setInputValue(e.target.value)}
        onBlur={submitCurrentTag}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === "Tab" || e.key === ",") {
            if (!inputValue.trim()) {
              return;
            }

            e.preventDefault();
            submitCurrentTag();
            return;
          }

          if (e.key === "Backspace" && !inputValue && mappingIds.length > 0) {
            const lastMappingId = mappingIds[mappingIds.length - 1];
            if (lastMappingId) {
              deleteTagMapping(lastMappingId);
            }
          }
        }}
      />
    </div>
  );
}

function useSessionTags(sessionId: string) {
  const store = useMainStore();
  const mappingIds = useSessionTagMappingIds(sessionId);

  const existingTagIdsByName = useMemo(() => {
    const byName = new Map<string, string>();
    for (const mappingId of mappingIds) {
      const tagId = store?.getCell("mapping_tag_session", mappingId, "tag_id");
      if (typeof tagId !== "string" || !tagId) {
        continue;
      }

      const tagName = store?.getCell("tags", tagId, "name");
      if (typeof tagName !== "string") {
        continue;
      }

      byName.set(tagName.toLowerCase(), tagId);
    }
    return byName;
  }, [mappingIds, store]);

  return { mappingIds, existingTagIdsByName };
}

function useAddTag(
  sessionId: string,
  existingTagIdsByName: Map<string, string>,
) {
  const store = useMainStore();

  return useCallback(
    (name: string) => {
      if (!store) {
        return;
      }
      const userId = store.getValue("user_id") as string | undefined;
      if (!userId) {
        return;
      }

      const normalized = name.toLowerCase();
      const existingTagId = existingTagIdsByName.get(normalized);

      let tagId = existingTagId;
      if (!tagId) {
        let foundTagId: string | null = null;
        store.forEachRow("tags", (rowId, _forEachCell) => {
          if (foundTagId) {
            return;
          }

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
      store.forEachRow("mapping_tag_session", (_rowId, _forEachCell) => {
        if (hasMapping) {
          return;
        }

        const currentTagId = store.getCell(
          "mapping_tag_session",
          _rowId,
          "tag_id",
        );
        const currentSessionId = store.getCell(
          "mapping_tag_session",
          _rowId,
          "session_id",
        );

        if (currentSessionId === sessionId && currentTagId === tagId) {
          hasMapping = true;
        }
      });

      if (!hasMapping) {
        store.setRow("mapping_tag_session", crypto.randomUUID(), {
          user_id: userId,
          session_id: sessionId,
          tag_id: tagId,
        });
      }
    },
    [existingTagIdsByName, sessionId, store],
  );
}

function normalizeTag(value: string): string {
  const trimmed = value.trim().replace(/^#+/, "").replace(/,+$/, "");
  return trimmed;
}
