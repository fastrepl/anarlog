import { type ReactNode } from "react";

import { cn } from "@hypr/utils";

export function Section({
  icon,
  title,
  action,
  children,
}: {
  icon: ReactNode;
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div
        className={cn([
          "bg-neutral-50 px-4 py-1",
          "flex items-center justify-between",
        ])}
      >
        <div className="flex items-center gap-2">
          {icon}
          <h3 className="text-sm font-medium">{title}</h3>
        </div>
        {action}
      </div>

      {children || (
        <div className="text-muted-foreground py-4 text-sm">Empty</div>
      )}
    </div>
  );
}
