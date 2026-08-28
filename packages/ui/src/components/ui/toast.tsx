import * as stylex from "@stylexjs/stylex";
import type { ComponentProps, CSSProperties } from "react";
import {
  Toaster as Sonner,
  toast as rawSonnerToast,
  type ExternalToast,
} from "sonner";

import { colors, radii, spacing } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

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

type ToasterProps = ComponentProps<typeof Sonner> & StyleXProps;

const Toaster = ({
  theme = "system",
  position = "bottom-right",
  richColors = true,
  closeButton = true,
  className,
  style,
  sx,
  toastOptions,
  ...props
}: ToasterProps) => {
  const classNames = toastOptions?.classNames;

  return (
    <Sonner
      {...props}
      theme={theme}
      position={position}
      richColors={richColors}
      closeButton={closeButton}
      toastOptions={{
        ...toastOptions,
        classNames: {
          ...classNames,
          toast: mergeStyleXProps(
            styles.toast,
            cn(["toast", classNames?.toast]),
          ).className,
          description: mergeStyleXProps(
            styles.description,
            classNames?.description,
          ).className,
          actionButton: mergeStyleXProps(
            styles.actionButton,
            classNames?.actionButton,
          ).className,
          cancelButton: mergeStyleXProps(
            styles.cancelButton,
            classNames?.cancelButton,
          ).className,
        },
      }}
      {...mergeStyleXProps([styles.toaster, sx], cn(["toaster", className]), {
        "--width": "300px",
        ...style,
      } as CSSProperties)}
    />
  );
};

const styles = stylex.create({
  toaster: {
    width: "300px",
  },
  toast: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    color: colors.foreground,
    gap: spacing.md,
    overflow: "visible",
    padding: "0.875rem",
    pointerEvents: "auto",
    width: "300px",
  },
  description: {
    color: colors.mutedForeground,
  },
  actionButton: {
    backgroundColor: colors.primary,
    color: colors.primaryForeground,
  },
  cancelButton: {
    backgroundColor: "transparent",
    color: colors.mutedForeground,
  },
});

export { Toaster };
