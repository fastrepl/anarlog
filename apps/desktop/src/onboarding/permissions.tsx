import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import {
  ArrowRightIcon,
  CheckIcon,
  MicIcon,
  MousePointer2Icon,
  type LucideIcon,
  Volume2Icon,
} from "lucide-react";
import { useRef } from "react";

import { type PermissionStatus } from "@hypr/plugin-permissions";
import { cn } from "@hypr/utils";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
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
  opensSettingsWhenDenied = true,
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
  opensSettingsWhenDenied?: boolean;
}) {
  const { t } = useLingui();
  const isAuthorized = status === "authorized";
  const opensSettings =
    isAuthorized || (opensSettingsWhenDenied && status === "denied");
  const title = isAuthorized ? enabledLabel : enableLabel;
  const body = isAuthorized ? enabledBody : enableBody;

  return (
    <button
      type="button"
      onClick={onAction}
      disabled={isPending || isAuthorized}
      title={body}
      className={cn([
        "group flex w-full min-w-0 items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-all",
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
      <span
        className={cn([
          "min-w-0 flex-1 truncate text-sm font-medium",
          isAuthorized ? "text-foreground" : "text-primary-foreground",
        ])}
      >
        {title}
      </span>
      {!isAuthorized && (
        <ArrowRightIcon className="text-primary-foreground/70 size-4 shrink-0 transition-transform group-hover:translate-x-0.5" />
      )}
    </button>
  );
}

function ContinueWhenComplete({
  onContinue,
  hasContinuedRef,
}: {
  onContinue?: () => void;
  hasContinuedRef: { current: boolean };
}) {
  useMountEffect(() => {
    if (hasContinuedRef.current) return;
    hasContinuedRef.current = true;
    onContinue?.();
  });

  return null;
}

function PermissionsSectionContent({
  onContinue,
  accessibility,
}: {
  onContinue?: () => void;
  accessibility?: ReturnType<typeof usePermission>;
}) {
  const { t } = useLingui();
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");
  const hasContinuedRef = useRef(false);

  const isComplete =
    mic.status === "authorized" &&
    systemAudio.status === "authorized" &&
    (!accessibility || accessibility.status === "authorized");

  const handleAction = (perm: ReturnType<typeof usePermission>) => {
    if (perm.status === "denied") {
      perm.open();
    } else {
      perm.request();
    }
  };

  return (
    <div>
      {isComplete && (
        <ContinueWhenComplete
          onContinue={onContinue}
          hasContinuedRef={hasContinuedRef}
        />
      )}

      <div className="flex flex-col gap-2">
        <PermissionBlock
          enabledLabel={t`Anarlog can hear your voice`}
          enableLabel={t`Help Anarlog listen to you`}
          enabledBody={t`Microphone access turned on`}
          enableBody={t`Use your microphone to capture your voice`}
          Icon={MicIcon}
          permissionName={t`Microphone`}
          status={mic.status}
          isPending={mic.isPending}
          onAction={() => handleAction(mic)}
        />

        <PermissionBlock
          enabledLabel={t`Anarlog can hear others`}
          enableLabel={t`Help Anarlog listen to others`}
          enabledBody={t`System audio enabled`}
          enableBody={t`Use system audio to capture other speakers`}
          Icon={Volume2Icon}
          permissionName={t`System audio`}
          status={systemAudio.status}
          isPending={systemAudio.isPending}
          onAction={() => handleAction(systemAudio)}
        />

        {accessibility && (
          <PermissionBlock
            enabledLabel={t`Anarlog can read meeting details`}
            enableLabel={t`Help Anarlog read meeting activity`}
            enabledBody={t`Meeting details access turned on`}
            enableBody={t`Read meeting controls, visible chat, and participant status`}
            Icon={MousePointer2Icon}
            permissionName={t`Accessibility`}
            status={accessibility.status}
            isPending={accessibility.isPending}
            onAction={accessibility.request}
            opensSettingsWhenDenied={false}
          />
        )}
      </div>
    </div>
  );
}

function MacOSPermissionsSection({ onContinue }: { onContinue?: () => void }) {
  const accessibility = usePermission("accessibility");

  return (
    <PermissionsSectionContent
      onContinue={onContinue}
      accessibility={accessibility}
    />
  );
}

export function PermissionsSection({
  onContinue,
}: {
  onContinue?: () => void;
}) {
  if (platform() === "macos") {
    return <MacOSPermissionsSection onContinue={onContinue} />;
  }

  return <PermissionsSectionContent onContinue={onContinue} />;
}
