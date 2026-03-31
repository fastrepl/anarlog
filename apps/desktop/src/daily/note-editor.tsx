import { useMemo } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";

import { type JSONContent, NoteEditor } from "~/editor/session";
import * as main from "~/store/tinybase/store/main";

export function DailyNoteEditor({ date }: { date: string }) {
  const content = main.UI.useCell(
    "daily_notes",
    date,
    "content",
    main.STORE_ID,
  );

  const initialContent = useMemo<JSONContent>(
    () => parseJsonContent(content as string),
    [content],
  );

  const handleChange = main.UI.useSetPartialRowCallback(
    "daily_notes",
    date,
    (input: JSONContent) => ({ content: JSON.stringify(input), date }),
    [date],
    main.STORE_ID,
  );

  return (
    <div className="px-2">
      <NoteEditor
        key={`daily-${date}`}
        initialContent={initialContent}
        handleChange={handleChange}
      />
    </div>
  );
}
