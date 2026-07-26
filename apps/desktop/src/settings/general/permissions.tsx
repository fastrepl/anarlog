import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { AlertCircleIcon, ArrowRightIcon, CheckIcon } from "lucide-react";

import type { PermissionStatus } from "@hypr/plugin-permissions";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { usePermission } from "~/shared/hooks/usePermissions";

function PermissionRow({
  title,
  description,
  status,
  isPending,
  error,
  onRequest,
  onOpen,
  runtimeCapability = false,
}: {
  title: string;
  description: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  error?: string | null;
  onRequest: () => void;
  onOpen: () => void;
  runtimeCapability?: boolean;
}) {
  const { t } = useLingui();
  const isAuthorized = status === "authorized";
  const isDenied = status === "denied";

  const handleButtonClick = () => {
    if (runtimeCapability) {
      if (!isAuthorized) onRequest();
      return;
    }

    if (isAuthorized || isDenied) {
      onOpen();
    } else {
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
          {!isAuthorized && <AlertCircleIcon className="size-4" />}
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
          <CheckIcon className="size-4" />
        ) : (
          <ArrowRightIcon className="size-5" />
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

  return (
    <PermissionGroup title={<Trans>Audio</Trans>}>
      <PermissionRow
        title={t`Microphone`}
        description={t`Required to record your voice during meetings and calls`}
        status={mic.status}
        isPending={mic.isPending}
        error={mic.error}
        onRequest={mic.request}
        onOpen={mic.open}
        runtimeCapability={runtimeCapabilities}
      />
      <PermissionRow
        title={t`System audio`}
        description={t`Required to capture other participants' voices in meetings`}
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

  return (
    <div className="flex flex-col gap-8">
      <AudioPermissions />

      <PermissionRow
        title={t`Accessibility`}
        description={t`Required to read meeting controls, visible chat, and participant status`}
        status={accessibility.status}
        isPending={accessibility.isPending}
        onRequest={accessibility.request}
        onOpen={accessibility.open}
      />

      <PermissionGroup title={<Trans>Others</Trans>}>
        <PermissionRow
          title={t`Calendar`}
          description={t`Required to sync Apple Calendar events into Anarlog`}
          status={calendar.status}
          isPending={calendar.isPending}
          onRequest={calendar.request}
          onOpen={calendar.open}
        />
      </PermissionGroup>
    </div>
  );
}
