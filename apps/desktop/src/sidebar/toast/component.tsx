import { cn } from "@hypr/utils";

import type { ToastAction, ToastType } from "./types";

export function Toast({
  toast,
  onDismiss,
}: {
  toast: ToastType;
  onDismiss?: () => void | Promise<void>;
}) {
  const actions = getActions(toast, onDismiss);
  const progress = getProgress(toast);

  return (
    <div className="overflow-visible p-1">
      <div
        className={cn([
          "relative z-50 inline-flex max-w-[calc(100vw-24px)] items-center gap-3 py-1.5 pr-1.5 pl-4",
          "rounded-full bg-white",
          "border shadow-lg",
          toast.variant === "error"
            ? "border-red-200 shadow-red-100"
            : "border-neutral-200",
        ])}
      >
        {toast.icon ? <span className="shrink-0">{toast.icon}</span> : null}

        <div
          className={cn([
            "max-w-50 truncate text-sm",
            toast.variant === "error" ? "text-red-500" : "text-neutral-600",
          ])}
        >
          {toast.description}
        </div>

        {progress !== null ? <ProgressPill progress={progress} /> : null}

        {actions.length > 0 ? (
          <div className="flex items-center gap-1">
            {actions.map((action, index) => (
              <button
                key={action.label}
                onClick={action.onClick}
                className={cn([
                  "flex items-center justify-center gap-1.5",
                  "rounded-full px-3 py-1.5 text-xs font-medium",
                  "whitespace-nowrap",
                  getActionClassName(toast, index),
                  "transition-colors",
                ])}
              >
                {action.icon}
                {action.label}
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function getActions(
  toast: ToastType,
  onDismiss: (() => void | Promise<void>) | undefined,
): ToastAction[] {
  if (toast.actions?.length) {
    return toast.actions;
  }

  const actions: ToastAction[] = [];

  if (toast.primaryAction) {
    actions.push(toast.primaryAction);
  }
  if (toast.secondaryAction) {
    actions.push(toast.secondaryAction);
  }
  if (onDismiss) {
    actions.push({ label: "Hide", onClick: onDismiss });
  }

  return actions;
}

function getActionClassName(toast: ToastType, index: number) {
  if (toast.variant === "error" && index === 0) {
    return "bg-red-50 text-red-500 hover:bg-red-100 hover:text-red-600";
  }

  if (index === 0) {
    return "bg-neutral-900 text-white hover:bg-neutral-800";
  }

  return "border border-neutral-200 bg-white text-neutral-700 hover:bg-neutral-50";
}

function getProgress(toast: ToastType) {
  if (toast.progress !== undefined) {
    return toast.progress;
  }

  if (!toast.downloads?.length) {
    return null;
  }

  const total = toast.downloads.reduce(
    (sum, download) => sum + download.progress,
    0,
  );

  return total / toast.downloads.length;
}

function ProgressPill({ progress }: { progress: number }) {
  return (
    <span className="rounded-full bg-neutral-100 px-2.5 py-1.5 text-xs font-medium whitespace-nowrap text-neutral-600">
      {Math.round(progress)}%
    </span>
  );
}
