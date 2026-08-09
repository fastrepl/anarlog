import type { ComponentProps, CSSProperties } from "react";
import {
  Toaster as Sonner,
  toast as rawSonnerToast,
  type ExternalToast,
} from "sonner";

export const TOAST_DURATIONS = {
  success: 3_000,
  info: 4_000,
  warning: 6_000,
  error: 8_000,
} as const;

function withDefaultDuration(
  duration: number,
  options?: ExternalToast,
): ExternalToast {
  return { duration, ...options };
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
        withDefaultDuration(TOAST_DURATIONS.error, options),
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
  style,
  ...props
}: ToasterProps) => (
  <Sonner
    theme={theme}
    position={position}
    richColors={richColors}
    closeButton={closeButton}
    className="toaster group"
    style={{ "--width": "300px", ...style } as CSSProperties}
    toastOptions={{
      classNames: {
        toast:
          "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border group-[.toaster]:border-border group-[.toaster]:shadow-md group-[.toaster]:rounded-xl group-[.toaster]:overflow-visible group-[.toaster]:w-[300px] group-[.toaster]:p-3.5 group-[.toaster]:gap-3",
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
