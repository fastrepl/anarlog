import { type ReactNode } from "react";

import { cn } from "@hypr/utils";

export function TemplateDetailScrollArea({
  children,
  className = "px-6 pb-6",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className="relative flex-1 overflow-hidden">
      <div className={cn(["scroll-fade-y h-full overflow-y-auto", className])}>
        {children}
      </div>
    </div>
  );
}
