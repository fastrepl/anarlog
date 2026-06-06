import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

export function SettingsAlert({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn([
        "bg-alert text-alert-foreground border-alert-border rounded-lg border px-4 py-3 text-sm",
        className,
      ])}
    >
      {children}
    </div>
  );
}
