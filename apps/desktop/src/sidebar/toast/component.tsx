import { X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import type { DownloadProgress, ToastType } from "./types";

export function Toast({
  toast,
  onDismiss,
  alwaysShowDismissButton = false,
}: {
  toast: ToastType;
  onDismiss?: () => void | Promise<void>;
  alwaysShowDismissButton?: boolean;
}) {
  const contentRef = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState<number | "auto">("auto");
  const [isAnimatingHeight, setIsAnimatingHeight] = useState(false);
  const contentKey = toast.actions ? "actions" : "default";

  useEffect(() => {
    if (!contentRef.current) return;
    const measure = () => {
      if (contentRef.current) {
        setHeight(contentRef.current.scrollHeight);
      }
    };
    measure();
    setIsAnimatingHeight(true);
    const ro = new ResizeObserver(measure);
    ro.observe(contentRef.current);
    return () => ro.disconnect();
  }, [contentKey]);

  return (
    <div className="overflow-visible p-1">
      <div
        className={cn([
          "group relative z-50 overflow-visible rounded-lg",
          "bg-white p-4",
          toast.variant === "error"
            ? "border border-red-300 shadow-xl shadow-red-200"
            : "border border-neutral-200 shadow-xl",
        ])}
      >
        {onDismiss && (
          <button
            onClick={onDismiss}
            aria-label="Dismiss toast"
            className={cn([
              "absolute top-1.5 right-1.5 z-10 flex size-6 items-center justify-center rounded-full",
              alwaysShowDismissButton
                ? "opacity-100"
                : "opacity-0 group-hover:opacity-50 hover:opacity-100!",
              "hover:bg-neutral-200",
              "transition-all duration-200",
            ])}
          >
            <X className="h-4 w-4" />
          </button>
        )}

        <motion.div
          animate={{ height }}
          onAnimationStart={() => setIsAnimatingHeight(true)}
          onAnimationComplete={() => setIsAnimatingHeight(false)}
          transition={{ duration: 0.25, ease: "easeInOut" }}
          style={{ overflow: isAnimatingHeight ? "hidden" : "visible" }}
        >
          <div ref={contentRef}>
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={contentKey}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.15, ease: "easeInOut" }}
                className="flex flex-col gap-2"
              >
                <div
                  className={cn(["flex flex-col gap-2", onDismiss && "pr-6"])}
                >
                  {(toast.icon || toast.title) && (
                    <div className="flex items-center gap-2">
                      {toast.icon}
                      {toast.title && (
                        <h3 className="text-lg font-bold text-neutral-900">
                          {toast.title}
                        </h3>
                      )}
                    </div>
                  )}

                  <div className="text-sm">{toast.description}</div>
                </div>

                <div className="mt-1 flex flex-col gap-2 overflow-visible">
                  {toast.progress !== undefined && (
                    <ProgressBar progress={toast.progress} />
                  )}
                  {toast.downloads && toast.downloads.length > 0 && (
                    <div className="flex flex-col gap-2">
                      {toast.downloads.map((download) => (
                        <DownloadProgressBar
                          key={download.model}
                          download={download}
                        />
                      ))}
                    </div>
                  )}
                  {toast.actions && toast.actions.length > 0 ? (
                    toast.actions.map((action) => (
                      <button
                        key={action.label}
                        onClick={action.onClick}
                        className={cn([
                          "flex w-full items-center justify-center gap-2",
                          "rounded-full bg-neutral-200 py-2 text-sm font-medium text-neutral-900",
                          "duration-150 hover:scale-[1.01] hover:bg-neutral-300 active:scale-[0.99]",
                        ])}
                      >
                        {action.icon}
                        {action.label}
                      </button>
                    ))
                  ) : (
                    <>
                      {toast.primaryAction && (
                        <button
                          onClick={toast.primaryAction.onClick}
                          className="flex h-11 w-full items-center justify-center rounded-full border-2 border-stone-600 bg-stone-800 px-4 text-sm font-medium text-white shadow-[0_2px_6px_rgba(87,83,78,0.22),0_10px_18px_-10px_rgba(87,83,78,0.65)] transition-all duration-200 hover:bg-stone-700"
                        >
                          {toast.primaryAction.label}
                        </button>
                      )}
                      {toast.secondaryAction && (
                        <button
                          onClick={toast.secondaryAction.onClick}
                          className="w-full rounded-full bg-neutral-200 py-2 text-sm font-medium text-neutral-900 duration-150 hover:scale-[1.01] active:scale-[0.99]"
                        >
                          {toast.secondaryAction.label}
                        </button>
                      )}
                    </>
                  )}
                </div>
              </motion.div>
            </AnimatePresence>
          </div>
        </motion.div>
      </div>
    </div>
  );
}

function ProgressBar({ progress }: { progress: number }) {
  return (
    <div className="relative w-full overflow-hidden rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 py-2">
      <div
        className="absolute inset-0 bg-linear-to-t from-stone-600 to-stone-500 transition-all duration-300"
        style={{ width: `${progress}%` }}
      />
      <span
        className={cn([
          "relative z-10 block text-center text-sm font-medium transition-colors duration-150",
          progress >= 48 ? "text-white" : "text-neutral-900",
        ])}
      >
        {Math.round(progress)}%
      </span>
    </div>
  );
}

function DownloadProgressBar({ download }: { download: DownloadProgress }) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between text-xs text-neutral-600">
        <span className="truncate font-medium">{download.displayName}</span>
        <span>{Math.round(download.progress)}%</span>
      </div>
      <div className="relative h-2 w-full overflow-hidden rounded-full bg-linear-to-t from-neutral-200 to-neutral-100">
        <div
          className="absolute inset-0 rounded-full bg-linear-to-t from-stone-600 to-stone-500 transition-all duration-300"
          style={{ width: `${download.progress}%` }}
        />
      </div>
    </div>
  );
}
