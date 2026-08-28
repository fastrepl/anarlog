import { useLingui } from "@lingui/react/macro";
import {
  ArrowRight,
  Check,
  Cursor,
  type Icon,
  Microphone,
  SpeakerHigh,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { type PermissionStatus } from "@anlg/plugin-permissions";

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
      data-permission-block
      type="button"
      onClick={onAction}
      disabled={isPending || isAuthorized}
      title={body}
      {...stylex.props([
        styles.permission,
        isAuthorized ? styles.authorized : styles.unauthorized,
        (isPending || isAuthorized) && styles.cursorDefault,
        isPending && styles.pending,
      ])}
      aria-label={
        opensSettings
          ? t`Open ${permissionName.toLowerCase()} settings`
          : (actionLabel ?? t`Enable ${permissionName.toLowerCase()}`)
      }
    >
      <div
        {...stylex.props([
          styles.iconContainer,
          isAuthorized
            ? styles.authorizedIconContainer
            : styles.unauthorizedIconContainer,
        ])}
      >
        {isAuthorized ? (
          <Check {...stylex.props(styles.icon)} />
        ) : (
          <Icon {...stylex.props(styles.icon)} />
        )}
      </div>
      <span
        {...stylex.props([
          styles.label,
          isAuthorized ? styles.authorizedLabel : styles.unauthorizedLabel,
        ])}
      >
        {title}
      </span>
      {!isAuthorized && (
        <ArrowRight
          {...stylex.props(styles.arrow)}
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

      <div {...stylex.props(styles.list)}>
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
                : t`Read meeting controls and visible chat`
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

const styles = stylex.create({
  arrow: {
    color: `color-mix(in srgb, ${colors.primaryForeground} 70%, transparent)`,
    flexShrink: 0,
    height: "1rem",
    transform: {
      default: "translateX(0)",
      ":is([data-permission-block]:hover *)": "translateX(0.125rem)",
    },
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  authorized: {
    backgroundColor: colors.card,
    borderColor: colors.border,
  },
  authorizedIconContainer: {
    color: "rgb(22 163 74)",
  },
  authorizedLabel: {
    color: colors.foreground,
  },
  cursorDefault: {
    cursor: "default",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  iconContainer: {
    alignItems: "center",
    borderRadius: radii.md,
    display: "flex",
    flexShrink: 0,
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  label: {
    flex: "1",
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  list: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  pending: {
    opacity: 0.5,
  },
  permission: {
    alignItems: "center",
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    minWidth: 0,
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  unauthorized: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderColor: colors.primary,
    boxShadow: "0 4px 14px rgb(87 83 78 / 0.18)",
    color: colors.primaryForeground,
    transform: {
      default: "scale(1)",
      ":active": "scale(0.98)",
    },
  },
  unauthorizedIconContainer: {
    backgroundColor: `color-mix(in srgb, ${colors.primaryForeground} 10%, transparent)`,
    color: colors.primaryForeground,
  },
  unauthorizedLabel: {
    color: colors.primaryForeground,
  },
});
