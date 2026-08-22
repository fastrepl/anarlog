import { cn } from "@anlg/utils";

import { StandardContentWrapper } from "~/shared/main";

export function SessionSurface({
  header,
  children,
  floatingButton,
  overlay,
}: {
  header?: React.ReactNode;
  children: React.ReactNode;
  floatingButton?: React.ReactNode;
  overlay?: React.ReactNode;
}) {
  return (
    <StandardContentWrapper floatingButton={floatingButton}>
      <div
        data-session-surface
        className="relative isolate flex h-full flex-col"
      >
        <div
          className={cn([
            "flex h-full min-h-0 flex-1 flex-col",
            overlay ? "relative z-0 overflow-hidden" : null,
          ])}
          {...(overlay ? { inert: true, "aria-hidden": true } : {})}
        >
          <div
            className={cn([
              "flex h-full min-h-0 flex-1 flex-col",
              overlay
                ? "origin-center scale-[1.03] blur-[22px] select-none"
                : null,
            ])}
          >
            {header ? (
              <div data-tauri-drag-region className="px-1">
                {header}
              </div>
            ) : null}
            <div className="min-h-0 flex-1 px-2">{children}</div>
          </div>
        </div>
        {overlay ? (
          <div className="absolute inset-0 z-10 overflow-hidden">{overlay}</div>
        ) : null}
      </div>
    </StandardContentWrapper>
  );
}
