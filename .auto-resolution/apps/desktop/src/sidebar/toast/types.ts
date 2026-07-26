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

export type ToastType = {
  id: string;
  icon?: ReactNode;
  description: ReactNode;
  primaryAction?: ToastAction;
  dismissible: boolean;
  variant?: "default" | "error" | "warning";
  loading?: boolean;
};

export type ToastCondition = () => boolean;
