import { useRef, type ReactNode } from "react";

import {
  getScrollFadeMaskClassName,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { cn } from "@hypr/utils";

export function TemplateDetailScrollArea({
  children,
  className = "px-6 pb-6",
}: {
  children: ReactNode;
  className?: string;
}) {
  const fadeRef = useRef<HTMLDivElement>(null);
  useScrollFade(fadeRef, "vertical");

  return (
    <div className="relative flex-1 overflow-hidden">
      <div
        ref={fadeRef}
        className={cn([
          "h-full overflow-y-auto",
          getScrollFadeMaskClassName("vertical"),
          className,
        ])}
      >
        {children}
      </div>
    </div>
  );
}
