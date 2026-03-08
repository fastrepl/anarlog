import { AlertCircle, CheckCircle, Download, RefreshCw, X } from "lucide-react";

import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { useOTA } from "./task";

import { MenuItem } from "~/sidebar/profile/shared";

export function UpdateChecker() {
  const {
    state,
    update,
    error,
    currentVersion,
    downloadProgress,
    handleCheckForUpdate,
    handleStartDownload,
    handleCancelDownload,
    handleInstall,
  } = useOTA();

  if (state === "checking") {
    return (
      <MenuItem
        icon={Spinner}
        label="Checking for updates..."
        onClick={() => {}}
      />
    );
  }

  if (state === "noUpdate") {
    return (
      <MenuItem
        icon={CheckCircle}
        label={currentVersion ? `v${currentVersion}` : "No updates available"}
        onClick={() => {}}
      />
    );
  }

  if (state === "error") {
    return (
      <MenuItem
        icon={AlertCircle}
        label={error || "Update failed"}
        badge={
          <button
            onClick={(e) => {
              e.stopPropagation();
              void handleCheckForUpdate();
            }}
            className={cn([
              "rounded-full",
              "px-2 py-0.5",
              "bg-red-50",
              "text-xs font-semibold text-red-600",
              "hover:bg-red-100",
            ])}
          >
            Retry
          </button>
        }
        onClick={() => {}}
      />
    );
  }

  if (state === "available") {
    return (
      <MenuItem
        icon={Download}
        label={`Download v${update?.version || "new version"}`}
        badge={<div className="h-2 w-2 rounded-full bg-red-500" />}
        onClick={handleStartDownload}
      />
    );
  }

  if (state === "downloading") {
    return (
      <MenuItemLikeContainer>
        <div className="flex w-full items-center justify-between gap-2.5">
          <div className="flex items-center gap-2.5">
            <svg className="h-4 w-4 shrink-0" viewBox="0 0 16 16">
              <circle
                cx="8"
                cy="8"
                r="6"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                className="text-neutral-200"
              />
              <circle
                cx="8"
                cy="8"
                r="6"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeDasharray={`${2 * Math.PI * 6}`}
                strokeDashoffset={`${2 * Math.PI * 6 * (1 - downloadProgress.percentage / 100)}`}
                strokeLinecap="round"
                className="text-neutral-500 transition-all duration-300"
                style={{
                  transform: "rotate(-90deg)",
                  transformOrigin: "center",
                }}
              />
            </svg>
            <span className="text-sm text-black">
              Downloading... ({Math.round(downloadProgress.percentage)}%)
            </span>
          </div>
          <button
            onClick={handleCancelDownload}
            className={cn([
              "flex h-6 w-6 shrink-0 items-center justify-center",
              "rounded-full",
              "hover:bg-neutral-100",
              "transition-colors",
            ])}
            title="Cancel download"
          >
            <X className="h-4 w-4 text-neutral-500" />
          </button>
        </div>
      </MenuItemLikeContainer>
    );
  }

  if (state === "ready") {
    return (
      <MenuItem
        icon={CheckCircle}
        label="Install update"
        badge={
          <div className="h-2 w-2 animate-pulse rounded-full bg-red-500" />
        }
        onClick={handleInstall}
      />
    );
  }

  if (state === "installing") {
    return <MenuItem icon={Spinner} label="Installing..." onClick={() => {}} />;
  }

  if (state === "idle") {
    return (
      <MenuItem
        icon={RefreshCw}
        label="Check for updates"
        onClick={handleCheckForUpdate}
      />
    );
  }
}

function MenuItemLikeContainer({ children }: { children: React.ReactNode }) {
  return (
    <div
      className={cn([
        "flex w-full items-center gap-2.5 rounded-lg",
        "px-4 py-1.5",
        "text-sm text-black",
        "transition-colors hover:bg-neutral-100",
      ])}
    >
      {children}
    </div>
  );
}
