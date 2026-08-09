import type { ReactNode } from "react";

export type ToastAction = {
  label: string;
  icon?: ReactNode;
  onClick: () => void | Promise<void>;
};

export type DownloadProgress = {
  model: string;
  displayName: string;
  progress: number;
};

export type ToastLifecycle =
  | { type: "condition-bound" }
  | {
      type: "persistent";
      dismissal: "permanent" | "session" | "day";
      dismissalId?: string;
    };

export type ToastType = {
  id: string;
  icon?: ReactNode;
  description: ReactNode;
  primaryAction?: ToastAction;
  lifecycle: ToastLifecycle;
  variant?: "default" | "error" | "warning";
  loading?: boolean;
};

export type ToastCondition = () => boolean;
