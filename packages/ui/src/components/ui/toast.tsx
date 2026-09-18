import type { ComponentProps, MouseEvent, ReactNode } from "react";

import { CircleNotch } from "@anlg/ui/components/icons";

import {
  AppToaster,
  dismissAppToast,
  dismissAppToasts,
  showAppToast,
  type AppToastTone,
} from "./app-toast";

export const TOAST_DURATIONS = {
  success: 3_000,
  info: 4_000,
  warning: 6_000,
  error: 5_000,
} as const;

type ToastOptions = {
  id?: string | number;
  description?: ReactNode;
  descriptionClassName?: string;
  duration?: number;
  dismissible?: boolean;
  closeButton?: boolean;
  icon?: ReactNode;
  action?: {
    label: ReactNode;
    onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  };
  onDismiss?: () => void;
};

let nextToastId = 0;

function showToast(
  message: ReactNode,
  options: ToastOptions = {},
  tone: AppToastTone | "loading" = "info",
) {
  const id = options.id ?? `notification-${++nextToastId}`;
  const requestedDuration = options.duration;
  const durationMs =
    tone === "error" && requestedDuration !== Infinity
      ? requestedDuration === undefined || !Number.isFinite(requestedDuration)
        ? TOAST_DURATIONS.error
        : Math.min(requestedDuration, TOAST_DURATIONS.error)
      : (requestedDuration ??
        (tone === "loading" ? Infinity : TOAST_DURATIONS[tone]));

  showAppToast({
    id: String(id),
    message,
    description: options.descriptionClassName ? (
      <div className={options.descriptionClassName}>{options.description}</div>
    ) : (
      options.description
    ),
    tone: tone === "loading" ? "info" : tone,
    leading:
      options.icon ??
      (tone === "loading" ? (
        <CircleNotch className="animate-spin" />
      ) : undefined),
    durationMs,
    dismissible: options.dismissible,
    closeButton: options.closeButton,
    action: options.action,
    onDismiss: (reason) => {
      if (reason === "user") options.onDismiss?.();
    },
  });
  return id;
}

export const toast = Object.assign(
  (message: ReactNode, options?: ToastOptions) => showToast(message, options),
  {
    message: (message: ReactNode, options?: ToastOptions) =>
      showToast(message, options),
    success: (message: ReactNode, options?: ToastOptions) =>
      showToast(message, options, "success"),
    info: (message: ReactNode, options?: ToastOptions) =>
      showToast(message, options, "info"),
    warning: (message: ReactNode, options?: ToastOptions) =>
      showToast(message, options, "warning"),
    error: (message: ReactNode, options?: ToastOptions) =>
      showToast(message, options, "error"),
    loading: (message: ReactNode, options?: ToastOptions) =>
      showToast(message, options, "loading"),
    dismiss: (id?: string | number) => {
      if (id === undefined) dismissAppToasts();
      else dismissAppToast(String(id));
    },
  },
);

export function Toaster({
  position = "bottom-right",
  ...props
}: Omit<ComponentProps<typeof AppToaster>, "position"> & {
  position?: "bottom-right" | "bottom-center" | "top-right" | "top-center";
}) {
  return (
    <AppToaster
      align={position.endsWith("right") ? "end" : "center"}
      position={position.startsWith("top") ? "top" : "bottom"}
      size="compact"
      {...props}
    />
  );
}
