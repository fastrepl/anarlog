import { useLingui } from "@lingui/react/macro";
import {
  ArrowRight,
  Check,
  Cursor,
  type Icon,
  Microphone,
  SpeakerHigh,
} from "@phosphor-icons/react";
import { platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";

import { type PermissionStatus } from "@anlg/plugin-permissions";
import { cn } from "@anlg/utils";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import {
  trackPermissionRequested,
  usePermissionAnalytics,
} from "~/shared/hooks/usePermissionAnalytics";
import {
  closePermissionAssistant,
  usePermission,
  usePermissionGuidance,
} from "~/shared/hooks/usePermissions";

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
  actionLabel,
  assisted = false,
  opensSettingsWhenDenied = true,
}: {
  enabledLabel: string;
  enableLabel: string;
  enabledBody: string;
  enableBody: string;
  Icon: Icon;
  permissionName: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  onAction: () => void;
  actionLabel?: string;
  assisted?: boolean;
  opensSettingsWhenDenied?: boolean;
}) {
  const { t } = useLingui();
  const isAuthorized = status === "authorized";
  const opensSettings =
    isAuthorized ||
    assisted ||
    (opensSettingsWhenDenied && status === "denied");
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
          : (actionLabel ?? t`Enable ${permissionName.toLowerCase()}`)
      }
    >
      <div
        className={cn([
          "flex size-6 shrink-0 items-center justify-center rounded-md",
          isAuthorized
            ? "text-green-600"
            : "bg-primary-foreground/10 text-primary-foreground",
        ])}
      >
        {isAuthorized ? (
          <Check className="size-3.5" />
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
        <ArrowRight
          className="text-primary-foreground/70 size-4 shrink-0 transition-transform group-hover:translate-x-0.5"
          data-testid="permission-action-arrow"
        />
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
  accessibilityGuidance,
  runtimeCapabilities = false,
}: {
  onContinue?: () => void;
  accessibility?: ReturnType<typeof usePermission>;
  accessibilityGuidance?: ReturnType<typeof usePermissionGuidance>;
  runtimeCapabilities?: boolean;
}) {
  const { t } = useLingui();
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");
  const hasContinuedRef = useRef(false);
  usePermissionAnalytics("microphone", mic.confirmedStatus, "onboarding");
  usePermissionAnalytics(
    "system_audio",
    systemAudio.confirmedStatus,
    "onboarding",
  );
  usePermissionAnalytics(
    "accessibility",
    accessibility?.confirmedStatus,
    "onboarding",
  );

  const isComplete =
    mic.status === "authorized" &&
    systemAudio.status === "authorized" &&
    (!accessibility || accessibility.status === "authorized");

  const handleAction = (
    permission: string,
    perm: ReturnType<typeof usePermission>,
    opensSettingsWhenDenied: boolean,
    assisted = false,
  ) => {
    // Assisted panes are granted by hand in System Settings; their request API
    // only prompts once, so every click after that would be a silent no-op.
    if (assisted || (opensSettingsWhenDenied && perm.status === "denied")) {
      trackPermissionRequested(
        permission,
        perm.status,
        "onboarding",
        "open_settings",
      );
      perm.open();
    } else {
      trackPermissionRequested(
        permission,
        perm.status,
        "onboarding",
        "request",
      );
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
          enableBody={mic.error ?? t`Use your microphone to capture your voice`}
          Icon={Microphone}
          permissionName={t`Microphone`}
          status={mic.status}
          isPending={mic.isPending}
          onAction={() => handleAction("microphone", mic, !runtimeCapabilities)}
          actionLabel={
            runtimeCapabilities && mic.status === "denied"
              ? `${t`Try again`}: ${t`Microphone`}`
              : undefined
          }
          opensSettingsWhenDenied={!runtimeCapabilities}
        />

        <PermissionBlock
          enabledLabel={t`Anarlog can hear others`}
          enableLabel={t`Help Anarlog listen to others`}
          enabledBody={t`System audio enabled`}
          enableBody={
            systemAudio.error ?? t`Use system audio to capture other speakers`
          }
          Icon={SpeakerHigh}
          permissionName={t`System audio`}
          status={systemAudio.status}
          isPending={systemAudio.isPending}
          onAction={() =>
            handleAction("system_audio", systemAudio, !runtimeCapabilities)
          }
          actionLabel={
            runtimeCapabilities && systemAudio.status === "denied"
              ? `${t`Try again`}: ${t`System audio`}`
              : undefined
          }
          opensSettingsWhenDenied={!runtimeCapabilities}
        />

        {accessibility && (
          <PermissionBlock
            enabledLabel={t`Anarlog can read meeting details`}
            enableLabel={t`Help Anarlog read meeting activity`}
            enabledBody={t`Meeting details access turned on`}
            enableBody={
              accessibilityGuidance
                ? t`Opens System Settings and guides you to add Anarlog to the ${accessibilityGuidance.paneTitle ?? "Privacy"} list`
                : t`Read meeting controls, visible chat, and participant status`
            }
            Icon={Cursor}
            permissionName={t`Accessibility`}
            status={accessibility.status}
            isPending={accessibility.isPending}
            onAction={() =>
              handleAction(
                "accessibility",
                accessibility,
                false,
                Boolean(accessibilityGuidance),
              )
            }
            assisted={Boolean(accessibilityGuidance)}
            opensSettingsWhenDenied={false}
          />
        )}
      </div>
    </div>
  );
}

function MacOSPermissionsSection({ onContinue }: { onContinue?: () => void }) {
  const accessibility = usePermission("accessibility");
  const accessibilityGuidance = usePermissionGuidance("accessibility");

  // Leaving onboarding while the assistant is up would strand its overlay on
  // top of System Settings with nothing left to dismiss it.
  useMountEffect(() => () => void closePermissionAssistant());

  return (
    <PermissionsSectionContent
      onContinue={onContinue}
      accessibility={accessibility}
      accessibilityGuidance={accessibilityGuidance}
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

  return (
    <PermissionsSectionContent onContinue={onContinue} runtimeCapabilities />
  );
}
