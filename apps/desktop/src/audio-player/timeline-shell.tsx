import type { MouseEvent, ReactNode } from "react";

import { cn } from "@hypr/utils";

export function TimelineMeta({ children }: { children: ReactNode }) {
  return (
    <div className="inline-flex shrink-0 items-center gap-1 font-mono text-xs text-neutral-600 tabular-nums select-none">
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
      className="w-full rounded-xl bg-neutral-50 select-none"
      onContextMenu={onContextMenu}
    >
      <div className={cn(["flex items-center gap-2 p-2", "w-full max-w-full"])}>
        {leading}
        {meta}
        <div className="min-w-0 flex-1">{main}</div>
      </div>
    </div>
  );
}
