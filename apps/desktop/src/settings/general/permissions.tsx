import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowRight, Check, WarningCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";

import { colors } from "@anlg/design-system/tokens.stylex";
import type { PermissionStatus } from "@anlg/plugin-permissions";
import { Button } from "@anlg/ui/components/ui/button";

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

function PermissionRow({
  title,
  description,
  status,
  isPending,
  error,
  permission,
  onRequest,
  onOpen,
  assisted = false,
  runtimeCapability = false,
}: {
  title: string;
  description: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  error?: string | null;
  permission: string;
  onRequest: () => void;
  onOpen: () => void;
  assisted?: boolean;
  runtimeCapability?: boolean;
}) {
  const { t } = useLingui();
  const isAuthorized = status === "authorized";
  const isDenied = status === "denied";

  const handleButtonClick = () => {
    if (runtimeCapability) {
      if (!isAuthorized) {
        trackPermissionRequested(permission, status, "settings", "request");
        onRequest();
      }
      return;
    }

    if (assisted || isAuthorized || isDenied) {
      trackPermissionRequested(permission, status, "settings", "open_settings");
      onOpen();
    } else {
      trackPermissionRequested(permission, status, "settings", "request");
      onRequest();
    }
  };

  return (
    <div {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.copy)}>
        <div
          {...stylex.props(
            styles.titleRow,
            !isAuthorized && styles.unauthorized,
          )}
        >
          {!isAuthorized && <WarningCircle {...stylex.props(styles.icon)} />}
          <h3 {...stylex.props(styles.title)}>{title}</h3>
        </div>
        <p {...stylex.props(styles.description)}>{description}</p>
        {error && (
          <p role="alert" {...stylex.props(styles.error)}>
            {error}
          </p>
        )}
      </div>
      <Button
        variant={isAuthorized ? "ghost" : "default"}
        size="icon"
        onClick={handleButtonClick}
        disabled={isPending || (runtimeCapability && isAuthorized)}
        sx={[styles.action, isAuthorized && styles.authorizedAction]}
        aria-label={
          runtimeCapability
            ? isDenied
              ? `${t`Try again`}: ${title}`
              : title
            : assisted || isAuthorized || isDenied
              ? t`Open ${title.toLowerCase()} settings`
              : t`Request ${title.toLowerCase()} permission`
        }
      >
        {isAuthorized ? (
          <Check {...stylex.props(styles.icon)} />
        ) : (
          <ArrowRight {...stylex.props(styles.arrowIcon)} />
        )}
      </Button>
    </div>
  );
}

function PermissionGroup({
  title,
  children,
}: {
  title: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div>
      <h3 {...stylex.props(styles.groupTitle)}>{title}</h3>
      <div {...stylex.props(styles.group)}>{children}</div>
    </div>
  );
}

export function Permissions() {
  if (platform() === "macos") {
    return <MacOSPermissions />;
  }

  return (
    <div {...stylex.props(styles.groups)}>
      <AudioPermissions runtimeCapabilities />
    </div>
  );
}

function AudioPermissions({
  runtimeCapabilities = false,
}: {
  runtimeCapabilities?: boolean;
}) {
  const { t } = useLingui();
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");
  usePermissionAnalytics("microphone", mic.confirmedStatus, "settings");
  usePermissionAnalytics(
    "system_audio",
    systemAudio.confirmedStatus,
    "settings",
  );

  return (
    <PermissionGroup title={<Trans>Audio</Trans>}>
      <PermissionRow
        permission="microphone"
        title={t`Microphone`}
        description={t`Record your voice in meetings and calls.`}
        status={mic.status}
        isPending={mic.isPending}
        error={mic.error}
        onRequest={mic.request}
        onOpen={mic.open}
        runtimeCapability={runtimeCapabilities}
      />
      <PermissionRow
        permission="system_audio"
        title={t`System audio`}
        description={t`Record other participants in meetings.`}
        status={systemAudio.status}
        isPending={systemAudio.isPending}
        error={systemAudio.error}
        onRequest={systemAudio.request}
        onOpen={systemAudio.open}
        runtimeCapability={runtimeCapabilities}
      />
    </PermissionGroup>
  );
}

function MacOSPermissions() {
  const { t } = useLingui();
  const calendar = usePermission("calendar");
  const accessibility = usePermission("accessibility");
  const accessibilityGuidance = usePermissionGuidance("accessibility");
  usePermissionAnalytics("calendar", calendar.confirmedStatus, "settings");
  usePermissionAnalytics(
    "accessibility",
    accessibility.confirmedStatus,
    "settings",
  );

  // Leaving settings while the assistant is up would strand its overlay on top
  // of System Settings with nothing left to dismiss it.
  useMountEffect(() => () => void closePermissionAssistant());

  return (
    <div {...stylex.props(styles.groups)}>
      <AudioPermissions />

      <PermissionRow
        permission="accessibility"
        title={t`Accessibility`}
        description={
          accessibilityGuidance
            ? t`Opens System Settings and guides you to add Anarlog to the ${accessibilityGuidance.paneTitle ?? "Privacy"} list.`
            : t`Read meeting controls and visible chat.`
        }
        status={accessibility.status}
        isPending={accessibility.isPending}
        onRequest={accessibility.request}
        onOpen={accessibility.open}
        assisted={Boolean(accessibilityGuidance)}
      />

      <PermissionGroup title={<Trans>Others</Trans>}>
        <PermissionRow
          permission="calendar"
          title={t`Calendar`}
          description={t`Show Apple Calendar events in Anarlog.`}
          status={calendar.status}
          isPending={calendar.isPending}
          onRequest={calendar.request}
          onOpen={calendar.open}
        />
      </PermissionGroup>
    </div>
  );
}

const styles = stylex.create({
  action: {
    height: "2rem",
    width: "2rem",
  },
  arrowIcon: {
    height: "1.25rem",
    width: "1.25rem",
  },
  authorizedAction: {
    backgroundColor: {
      default: null,
      ":hover": "transparent",
    },
    color: {
      default: "rgb(22 163 74)",
      ":hover": "rgb(22 163 74)",
    },
  },
  copy: {
    flex: "1",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  error: {
    color: "rgb(239 68 68)",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
  },
  group: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  groups: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
  groupTitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 600,
    letterSpacing: "0.025em",
    lineHeight: "1rem",
    marginBottom: "0.75rem",
    textTransform: "uppercase",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  row: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  titleRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.25rem",
  },
  unauthorized: {
    color: "rgb(239 68 68)",
  },
});
