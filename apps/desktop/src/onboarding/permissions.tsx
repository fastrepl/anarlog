import { AlertCircleIcon, ArrowRightIcon, CheckIcon } from "lucide-react";

import { type PermissionStatus } from "@hypr/plugin-permissions";
import { cn } from "@hypr/utils";

import { usePermission } from "~/shared/hooks/usePermissions";

function PermissionBlock({
  enabledLabel,
  enableLabel,
  permissionName,
  status,
  isPending,
  onAction,
}: {
  enabledLabel: string;
  enableLabel: string;
  permissionName: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  onAction: () => void;
}) {
  const isAuthorized = status === "authorized";
  const opensSettings = isAuthorized || status === "denied";
  const title = isAuthorized ? enabledLabel : enableLabel;
  const body = isAuthorized
    ? "Open settings"
    : opensSettings
      ? "Open System Settings to finish setup"
      : "Allow access to continue";
  const ctaLabel = isAuthorized
    ? "Manage"
    : opensSettings
      ? "Open settings"
      : "Allow access";

  return (
    <button
      type="button"
      onClick={onAction}
      disabled={isPending}
      className={cn([
        "group flex min-w-0 flex-1 basis-0 items-center gap-3 rounded-xl px-3 py-3 text-left transition-all",
        isAuthorized
          ? "border border-neutral-200 hover:border-neutral-300 hover:bg-stone-50 active:scale-[0.98]"
          : "border border-red-200 bg-red-50 hover:bg-red-100/60 active:scale-[0.98]",
        isPending && "cursor-not-allowed opacity-50",
      ])}
      aria-label={
        opensSettings
          ? `Open ${permissionName.toLowerCase()} settings`
          : `Enable ${permissionName.toLowerCase()}`
      }
    >
      <div
        className={cn([
          "flex size-6 shrink-0 items-center justify-center rounded-md",
          isAuthorized
            ? "bg-stone-100 text-stone-600"
            : "bg-linear-to-t from-red-600 to-red-500 text-white",
        ])}
      >
        {isAuthorized ? (
          <CheckIcon className="size-3.5" />
        ) : (
          <AlertCircleIcon className="size-3.5" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <span
          className={cn([
            "text-sm font-medium",
            isAuthorized ? "text-neutral-900" : "text-red-600",
          ])}
        >
          {title}
        </span>
        <p className="hidden truncate text-xs text-neutral-500 @[480px]:block">
          {body}
        </p>
      </div>
      <div
        className={cn([
          "hidden shrink-0 items-center gap-1 text-xs font-medium @[480px]:inline-flex",
          isAuthorized ? "text-neutral-500" : "text-red-600",
        ])}
      >
        <span>{ctaLabel}</span>
        <ArrowRightIcon className="size-3.5 transition-transform group-hover:translate-x-0.5" />
      </div>
    </button>
  );
}

export function PermissionsSection() {
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");

  const handleAction = (perm: ReturnType<typeof usePermission>) => {
    if (perm.status === "denied") {
      perm.open();
    } else {
      perm.request();
    }
  };

  return (
    <div className="@container flex items-stretch gap-3">
      <PermissionBlock
        enabledLabel="Char can hear your voice"
        enableLabel="Help Char listen to your voice"
        permissionName="Microphone"
        status={mic.status}
        isPending={mic.isPending}
        onAction={() => handleAction(mic)}
      />

      <PermissionBlock
        enabledLabel="Char can hear others talk"
        enableLabel="Help Char listen to others talk"
        permissionName="System audio"
        status={systemAudio.status}
        isPending={systemAudio.isPending}
        onAction={() => handleAction(systemAudio)}
      />
    </div>
  );
}
