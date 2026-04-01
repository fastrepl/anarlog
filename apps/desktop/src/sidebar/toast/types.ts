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
  title?: string;
  description: ReactNode;
  primaryAction?: ToastAction;
  secondaryAction?: ToastAction;
  actions?: ToastAction[];
  dismissible: boolean;
  progress?: number;
  downloads?: DownloadProgress[];
  variant?: "default" | "error";
  gradient?: string;
};

export type ToastCondition = () => boolean;
