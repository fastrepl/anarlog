import {
  ArrowRightIcon,
  CheckIcon,
  MicIcon,
  type LucideIcon,
  Volume2Icon,
} from "lucide-react";
import { useEffect, useRef } from "react";

import { type PermissionStatus } from "@hypr/plugin-permissions";
import { cn } from "@hypr/utils";

import { usePermission } from "~/shared/hooks/usePermissions";

function PermissionBlock({
  enabledLabel,
  enableLabel,
  Icon,
  permissionName,
  status,
  isPending,
  onAction,
}: {
  enabledLabel: string;
  enableLabel: string;
  Icon: LucideIcon;
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
      disabled={isPending || isAuthorized}
      className={cn([
        "group flex min-w-0 flex-1 basis-0 items-center gap-3 rounded-xl px-3 py-3 text-left transition-all",
        isAuthorized
          ? "border border-neutral-200 bg-white"
          : "border border-stone-600 bg-stone-800 text-white shadow-[0_4px_14px_rgba(87,83,78,0.18)] hover:bg-stone-700 active:scale-[0.98]",
        (isPending || isAuthorized) && "cursor-default",
        isPending && "opacity-50",
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
            ? "bg-green-50 text-green-600"
            : "bg-white/10 text-white",
        ])}
      >
        {isAuthorized ? (
          <CheckIcon className="size-3.5" />
        ) : (
          <Icon className="size-3.5" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <span
          className={cn([
            "text-sm font-medium",
            isAuthorized ? "text-neutral-900" : "text-white",
          ])}
        >
          {title}
        </span>
        <p
          className={cn([
            "hidden truncate text-xs @[480px]:block",
            isAuthorized ? "text-neutral-500" : "text-white/70",
          ])}
        >
          {body}
        </p>
      </div>
      {!isAuthorized && (
        <div className="inline-flex shrink-0 items-center gap-1 text-xs font-medium text-white/80">
          <span className="hidden @[480px]:inline">{ctaLabel}</span>
          <ArrowRightIcon className="size-3.5 transition-transform group-hover:translate-x-0.5" />
        </div>
      )}
    </button>
  );
}

export function PermissionsSection({
  onContinue,
}: {
  onContinue?: () => void;
}) {
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");
  const hasContinuedRef = useRef(false);

  const isComplete =
    mic.status === "authorized" && systemAudio.status === "authorized";

  useEffect(() => {
    if (!isComplete || hasContinuedRef.current) return;
    hasContinuedRef.current = true;
    onContinue?.();
  }, [isComplete, onContinue]);

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
        Icon={MicIcon}
        permissionName="Microphone"
        status={mic.status}
        isPending={mic.isPending}
        onAction={() => handleAction(mic)}
      />

      <PermissionBlock
        enabledLabel="Char can hear others"
        enableLabel="Help Char listen to others"
        Icon={Volume2Icon}
        permissionName="System audio"
        status={systemAudio.status}
        isPending={systemAudio.isPending}
        onAction={() => handleAction(systemAudio)}
      />
    </div>
  );
}
