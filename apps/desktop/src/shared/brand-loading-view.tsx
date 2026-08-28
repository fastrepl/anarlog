import { cn } from "@anlg/utils";

import { AnarlogMark } from "./anarlog-mark";

export function BrandLoadingView({ detail }: { detail?: string }) {
  return (
    <div
      role="status"
      aria-label="Loading"
      data-tauri-drag-region
      className={cn([
        "bg-background flex h-screen w-screen items-center justify-center",
      ])}
    >
      <div className="flex flex-col items-center">
        <div
          className={cn([
            "relative w-[4.5rem]",
            "drop-shadow-[0_10px_24px_rgba(0,0,0,0.10)]",
            "dark:drop-shadow-[0_12px_28px_rgba(0,0,0,0.45)]",
          ])}
        >
          <AnarlogMark className="text-foreground/20 w-full" />
          <div
            aria-hidden="true"
            className={cn([
              "pointer-events-none absolute inset-0",
              "text-foreground/75 dark:text-foreground/50",
              "[-webkit-mask-image:linear-gradient(105deg,transparent_36%,#000_50%,transparent_64%)]",
              "[mask-image:linear-gradient(105deg,transparent_36%,#000_50%,transparent_64%)]",
              "[-webkit-mask-size:220%_100%]",
              "[mask-size:220%_100%]",
              "animate-logo-shimmer-sweep",
              "motion-reduce:animate-none motion-reduce:opacity-0",
            ])}
          >
            <AnarlogMark className="w-full" />
          </div>
        </div>
        {detail ? (
          <p className="text-muted-foreground mt-5 max-w-64 text-center text-xs">
            {detail}
          </p>
        ) : null}
      </div>
    </div>
  );
}
