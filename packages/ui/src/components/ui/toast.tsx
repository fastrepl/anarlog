import type { ComponentProps, CSSProperties } from "react";
import {
  Toaster as Sonner,
  toast as rawSonnerToast,
  type ExternalToast,
} from "sonner";

import {
  CheckCircle,
  CircleNotch,
  Info,
  Warning,
  WarningCircle,
  X,
} from "@anlg/ui/components/icons";

export const TOAST_DURATIONS = {
  success: 3_000,
  info: 4_000,
  warning: 6_000,
  error: 5_000,
} as const;

function withDefaultDuration(
  duration: number,
  options?: ExternalToast,
): ExternalToast {
  return { duration, ...options };
}

function withAutoDismissDuration(
  duration: number,
  options?: ExternalToast,
): ExternalToast {
  const requested = options?.duration;
  return {
    ...options,
    duration:
      requested === undefined || !Number.isFinite(requested)
        ? duration
        : Math.min(requested, duration),
  };
}

export const sonnerToast: typeof rawSonnerToast = Object.assign(
  (message: Parameters<typeof rawSonnerToast>[0], options?: ExternalToast) =>
    rawSonnerToast(message, withDefaultDuration(TOAST_DURATIONS.info, options)),
  rawSonnerToast,
  {
    success: (
      message: Parameters<typeof rawSonnerToast.success>[0],
      options?: ExternalToast,
    ) =>
      rawSonnerToast.success(
        message,
        withDefaultDuration(TOAST_DURATIONS.success, options),
      ),
    info: (
      message: Parameters<typeof rawSonnerToast.info>[0],
      options?: ExternalToast,
    ) =>
      rawSonnerToast.info(
        message,
        withDefaultDuration(TOAST_DURATIONS.info, options),
      ),
    warning: (
      message: Parameters<typeof rawSonnerToast.warning>[0],
      options?: ExternalToast,
    ) =>
      rawSonnerToast.warning(
        message,
        withDefaultDuration(TOAST_DURATIONS.warning, options),
      ),
    error: (
      message: Parameters<typeof rawSonnerToast.error>[0],
      options?: ExternalToast,
    ) =>
      rawSonnerToast.error(
        message,
        withAutoDismissDuration(TOAST_DURATIONS.error, options),
      ),
    message: (
      message: Parameters<typeof rawSonnerToast.message>[0],
      options?: ExternalToast,
    ) =>
      rawSonnerToast.message(
        message,
        withDefaultDuration(TOAST_DURATIONS.info, options),
      ),
  },
);

type ToasterProps = ComponentProps<typeof Sonner>;

const Toaster = ({
  theme = "system",
  position = "bottom-right",
  richColors = true,
  closeButton = true,
  icons,
  style,
  ...props
}: ToasterProps) => (
  <Sonner
    theme={theme}
    position={position}
    richColors={richColors}
    closeButton={closeButton}
    icons={{
      success: <CheckCircle size={20} aria-hidden="true" />,
      info: <Info size={20} aria-hidden="true" />,
      warning: <Warning size={20} aria-hidden="true" />,
      error: <WarningCircle size={20} aria-hidden="true" />,
      loading: (
        <CircleNotch size={20} className="animate-spin" aria-hidden="true" />
      ),
      close: <X size={12} aria-hidden="true" />,
      ...icons,
    }}
    className="toaster group"
    style={{ "--width": "300px", ...style } as CSSProperties}
    toastOptions={{
      classNames: {
        toast:
          "group toast pointer-events-auto group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border group-[.toaster]:border-border group-[.toaster]:shadow-md group-[.toaster]:rounded-xl group-[.toaster]:overflow-visible group-[.toaster]:w-[300px] group-[.toaster]:p-3.5 group-[.toaster]:gap-3",
        description: "group-[.toast]:text-muted-foreground",
        actionButton:
          "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
        cancelButton:
          "group-[.toast]:bg-transparent group-[.toast]:text-muted-foreground",
      },
    }}
    {...props}
  />
);

export { Toaster };
