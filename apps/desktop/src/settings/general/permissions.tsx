import { Trans, useLingui } from "@lingui/react/macro";
import { AlertCircleIcon, ArrowRightIcon, CheckIcon } from "lucide-react";
import { useState } from "react";

import type { PermissionStatus } from "@hypr/plugin-permissions";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { usePermission } from "~/shared/hooks/usePermissions";

function ActionLink({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn([
        "hover:text-foreground underline transition-colors",
        disabled && "cursor-not-allowed opacity-50",
      ])}
    >
      {children}
    </button>
  );
}

function PermissionRow({
  title,
  description,
  status,
  isPending,
  onRequest,
  onReset,
  onOpen,
}: {
  title: string;
  description: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  onRequest: () => void;
  onReset: () => void;
  onOpen: () => void;
}) {
  const { t } = useLingui();
  const [showActions, setShowActions] = useState(false);
  const isAuthorized = status === "authorized";
  const isDenied = status === "denied";

  const handleButtonClick = () => {
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
        <div className="text-muted-foreground text-xs">
          {!showActions ? (
            <div>
              {!isAuthorized && <span>{description} · </span>}
              <button
                type="button"
                onClick={() => setShowActions(true)}
                className="hover:text-foreground underline transition-colors"
              >
                <Trans>Having trouble?</Trans>
              </button>
            </div>
          ) : (
            <div>
              <Trans>You can</Trans>{" "}
              <ActionLink onClick={onRequest} disabled={isPending}>
                <Trans>Request,</Trans>
              </ActionLink>{" "}
              <ActionLink onClick={onReset} disabled={isPending}>
                <Trans>Reset</Trans>
              </ActionLink>{" "}
              <Trans>or</Trans>{" "}
              <ActionLink onClick={onOpen} disabled={isPending}>
                <Trans>Open</Trans>
              </ActionLink>{" "}
              <Trans>permission panel.</Trans>
            </div>
          )}
        </div>
      </div>
      <Button
        variant={isAuthorized ? "outline" : "default"}
        size="icon"
        onClick={handleButtonClick}
        disabled={isPending}
        className={cn([
          "size-8",
          isAuthorized && "bg-muted text-foreground hover:bg-accent",
        ])}
        aria-label={
          isAuthorized
            ? t`Open ${title.toLowerCase()} settings`
            : t`Request ${title.toLowerCase()} permission`
        }
      >
        {isAuthorized ? (
          <CheckIcon className="size-5" />
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
  const { t } = useLingui();
  const calendar = usePermission("calendar");
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");
  const accessibility = usePermission("accessibility");

  return (
    <div className="flex flex-col gap-8">
      <PermissionGroup title={<Trans>Audio</Trans>}>
        <PermissionRow
          title={t`Microphone`}
          description={t`Required to record your voice during meetings and calls`}
          status={mic.status}
          isPending={mic.isPending}
          onRequest={mic.request}
          onReset={mic.reset}
          onOpen={mic.open}
        />
        <PermissionRow
          title={t`System audio`}
          description={t`Required to capture other participants' voices in meetings`}
          status={systemAudio.status}
          isPending={systemAudio.isPending}
          onRequest={systemAudio.request}
          onReset={systemAudio.reset}
          onOpen={systemAudio.open}
        />
      </PermissionGroup>

      <PermissionRow
        title={t`Accessibility`}
        description={t`Required to detect meeting apps and sync mute status`}
        status={accessibility.status}
        isPending={accessibility.isPending}
        onRequest={accessibility.request}
        onReset={accessibility.reset}
        onOpen={accessibility.open}
      />

      <PermissionGroup title={<Trans>Others</Trans>}>
        <PermissionRow
          title={t`Calendar`}
          description={t`Required to sync Apple Calendar events into Anarlog`}
          status={calendar.status}
          isPending={calendar.isPending}
          onRequest={calendar.request}
          onReset={calendar.reset}
          onOpen={calendar.open}
        />
      </PermissionGroup>
    </div>
  );
}
