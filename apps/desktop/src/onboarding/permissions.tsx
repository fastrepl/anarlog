import { useLingui } from "@lingui/react/macro";
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
  enabledBody,
  enableBody,
  Icon,
  permissionName,
  status,
  isPending,
  onAction,
}: {
  enabledLabel: string;
  enableLabel: string;
  enabledBody: string;
  enableBody: string;
  Icon: LucideIcon;
  permissionName: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  onAction: () => void;
}) {
  const { t } = useLingui();
  const isAuthorized = status === "authorized";
  const opensSettings = isAuthorized || status === "denied";
  const title = isAuthorized ? enabledLabel : enableLabel;
  const body = isAuthorized ? enabledBody : enableBody;
  const ctaLabel = isAuthorized
    ? t`Manage`
    : opensSettings
      ? t`Open settings`
      : t`Allow access`;

  return (
    <button
      type="button"
      onClick={onAction}
      disabled={isPending || isAuthorized}
      className={cn([
        "group flex min-w-0 flex-1 basis-0 items-center gap-3 rounded-xl px-3 py-3 text-left transition-all",
        isAuthorized
          ? "border-border bg-card border"
          : "border-primary bg-primary text-primary-foreground hover:bg-primary/90 border shadow-[0_4px_14px_rgba(87,83,78,0.18)] active:scale-[0.98]",
        (isPending || isAuthorized) && "cursor-default",
        isPending && "opacity-50",
      ])}
      aria-label={
        opensSettings
          ? t`Open ${permissionName.toLowerCase()} settings`
          : t`Enable ${permissionName.toLowerCase()}`
      }
    >
      <div
        className={cn([
          "flex size-6 shrink-0 items-center justify-center rounded-md",
          isAuthorized
            ? "bg-green-50 text-green-600"
            : "bg-primary-foreground/10 text-primary-foreground",
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
            isAuthorized ? "text-foreground" : "text-primary-foreground",
          ])}
        >
          {title}
        </span>
        <p
          className={cn([
            "truncate text-xs @[480px]:block",
            isAuthorized
              ? "text-muted-foreground"
              : "text-primary-foreground/70",
          ])}
        >
          {body}
        </p>
      </div>
      {!isAuthorized && (
        <div className="text-primary-foreground/80 inline-flex shrink-0 items-center gap-1 text-xs font-medium">
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
  const { t } = useLingui();
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
        enabledLabel={t`Anarlog can hear your voice`}
        enableLabel={t`Allow microphone access`}
        enabledBody={t`Microphone access turned on`}
        enableBody={t`Help Anarlog listen to you`}
        Icon={MicIcon}
        permissionName={t`Microphone`}
        status={mic.status}
        isPending={mic.isPending}
        onAction={() => handleAction(mic)}
      />

      <PermissionBlock
        enabledLabel={t`Anarlog can hear others`}
        enableLabel={t`Allow system audio access`}
        enabledBody={t`System audio enabled`}
        enableBody={t`Help Anarlog listen to others`}
        Icon={Volume2Icon}
        permissionName={t`System audio`}
        status={systemAudio.status}
        isPending={systemAudio.isPending}
        onAction={() => handleAction(systemAudio)}
      />
    </div>
  );
}
