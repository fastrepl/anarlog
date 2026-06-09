import type { MouseEvent, ReactNode } from "react";

import { cn } from "@hypr/utils";

export function TimelineMeta({ children }: { children: ReactNode }) {
  return (
    <div className="text-muted-foreground inline-flex shrink-0 items-center gap-1 font-mono text-xs tabular-nums select-none">
      {children}
    </div>
  );
}

export function TimelineShell({
  leading,
  meta,
  main,
  onContextMenu,
}: {
  leading: ReactNode;
  meta?: ReactNode;
  main: ReactNode;
  onContextMenu?: (event: MouseEvent<HTMLDivElement>) => void;
}) {
  return (
    <div
      className="w-full rounded-xl bg-transparent select-none"
      onContextMenu={onContextMenu}
    >
      <div
        className={cn([
          "flex items-center gap-2 px-2 py-1",
          "w-full max-w-full",
        ])}
      >
        {leading}
        {meta}
        <div className="min-w-0 flex-1">{main}</div>
      </div>
    </div>
  );
}
