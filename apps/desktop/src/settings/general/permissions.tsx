import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowRight, Check, WarningCircle } from "@phosphor-icons/react";
import { platform } from "@tauri-apps/plugin-os";

import type { PermissionStatus } from "@anlg/plugin-permissions";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import {
  trackPermissionRequested,
  usePermissionAnalytics,
} from "~/shared/hooks/usePermissionAnalytics";
import { usePermission } from "~/shared/hooks/usePermissions";

function PermissionRow({
  title,
  description,
  status,
  isPending,
  error,
  permission,
  onRequest,
  onOpen,
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

    if (isAuthorized || isDenied) {
      trackPermissionRequested(permission, status, "settings", "open_settings");
      onOpen();
    } else {
      trackPermissionRequested(permission, status, "settings", "request");
      onRequest();
    }
  };

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <div
          className={cn([
            "mb-1 flex items-center gap-2",
            !isAuthorized && "text-red-500",
          ])}
        >
          {!isAuthorized && <WarningCircle className="size-4" />}
          <h3 className="text-sm font-medium">{title}</h3>
        </div>
        <p className="text-muted-foreground text-xs">{description}</p>
        {error && (
          <p role="alert" className="mt-1 text-xs text-red-500">
            {error}
          </p>
        )}
      </div>
      <Button
        variant={isAuthorized ? "ghost" : "default"}
        size="icon"
        onClick={handleButtonClick}
        disabled={isPending || (runtimeCapability && isAuthorized)}
        className={cn([
          "size-8",
          isAuthorized &&
            "text-green-600 hover:bg-transparent hover:text-green-600",
        ])}
        aria-label={
          runtimeCapability
            ? isDenied
              ? `${t`Try again`}: ${title}`
              : title
            : isAuthorized || isDenied
              ? t`Open ${title.toLowerCase()} settings`
              : t`Request ${title.toLowerCase()} permission`
        }
      >
        {isAuthorized ? (
          <Check className="size-4" />
        ) : (
          <ArrowRight className="size-5" />
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
      <h3 className="text-muted-foreground mb-3 text-xs font-semibold tracking-wide uppercase">
        {title}
      </h3>
      <div className="flex flex-col gap-4">{children}</div>
    </div>
  );
}

export function Permissions() {
  if (platform() === "macos") {
    return <MacOSPermissions />;
  }

  return (
    <div className="flex flex-col gap-8">
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
  usePermissionAnalytics("calendar", calendar.confirmedStatus, "settings");
  usePermissionAnalytics(
    "accessibility",
    accessibility.confirmedStatus,
    "settings",
  );

  return (
    <div className="flex flex-col gap-8">
      <AudioPermissions />

      <PermissionRow
        permission="accessibility"
        title={t`Accessibility`}
        description={t`Read meeting controls, chat, and participant status.`}
        status={accessibility.status}
        isPending={accessibility.isPending}
        onRequest={accessibility.request}
        onOpen={accessibility.open}
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
