import { useLingui } from "@lingui/react/macro";
import { useEffect, useState } from "react";

import { cn } from "@anlg/utils";

import {
  updateFolderInstructions,
  useFolderInstructions,
} from "~/session/folder-catalog";

export function FolderInstructionsField({
  folderPath,
  rows = 2,
}: {
  folderPath: string;
  rows?: number;
}) {
  const { t } = useLingui();
  const saved = useFolderInstructions(folderPath);
  const [value, setValue] = useState(saved);

  useEffect(() => {
    setValue(saved);
  }, [folderPath, saved]);

  return (
    <textarea
      aria-label={t`Folder instructions`}
      value={value}
      placeholder={t`How chat should use notes in this folder`}
      rows={rows}
      className={cn([
        "border-border/60 placeholder:text-muted-foreground w-full resize-none rounded-md border bg-transparent px-1.5 py-1",
        "focus-visible:ring-ring focus-visible:ring-1 focus-visible:outline-hidden",
        rows > 2 ? "text-sm leading-5" : "text-[11px] leading-4",
      ])}
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => {
        if (value === saved) {
          return;
        }
        void updateFolderInstructions(folderPath, value);
      }}
    />
  );
}
