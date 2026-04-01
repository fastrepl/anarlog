import * as React from "react";

import { cn } from "@hypr/utils";

export const NoteTab = React.forwardRef<
  HTMLButtonElement,
  React.ComponentPropsWithoutRef<"button"> & {
    isActive: boolean;
  }
>(({ isActive, className, children, type = "button", ...props }, ref) => {
  return (
    <button
      ref={ref}
      type={type}
      {...props}
      className={cn([
        "relative my-2 shrink-0 border-b-2 px-1 py-0.5 text-xs font-medium transition-all duration-200 select-none",
        isActive
          ? ["border-neutral-900", "text-neutral-900"]
          : [
              "border-transparent",
              "text-neutral-600",
              "hover:text-neutral-800",
            ],
        className,
      ])}
    >
      <span className="flex h-5 items-center gap-1">{children}</span>
    </button>
  );
});

NoteTab.displayName = "NoteTab";
