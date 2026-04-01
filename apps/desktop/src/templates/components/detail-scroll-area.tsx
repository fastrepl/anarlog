import { useRef, type ReactNode } from "react";

import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";

export function TemplateDetailScrollArea({
  children,
  className = "px-6 pb-6",
}: {
  children: ReactNode;
  className?: string;
}) {
  const fadeRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(fadeRef, "vertical");

  return (
    <div className="relative flex-1 overflow-hidden">
      <div ref={fadeRef} className={`h-full overflow-y-auto ${className}`}>
        {children}
      </div>
      {!atStart && <ScrollFadeOverlay position="top" />}
      {!atEnd && <ScrollFadeOverlay position="bottom" />}
    </div>
  );
}
