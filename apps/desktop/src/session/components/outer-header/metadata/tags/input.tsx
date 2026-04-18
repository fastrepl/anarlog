import { useCallback, useRef, useState } from "react";

import { TagChip } from "./chip";

import {
  useAddSessionTag,
  useSessionTagNameMap,
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
  const mappingIds = useSessionTagMappingIds(sessionId);
  const existingTagIdsByName = useSessionTagNameMap(sessionId);

  return { mappingIds, existingTagIdsByName };
}

function useAddTag(
  sessionId: string,
  _existingTagIdsByName: Map<string, string>,
) {
  return useAddSessionTag(sessionId);
}

function normalizeTag(value: string): string {
  const trimmed = value.trim().replace(/^#+/, "").replace(/,+$/, "");
  return trimmed;
}
