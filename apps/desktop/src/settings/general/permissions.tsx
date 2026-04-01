import { platform } from "@tauri-apps/plugin-os";
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
        "underline transition-colors hover:text-neutral-900",
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
        <div className="text-xs text-neutral-600">
          {!showActions ? (
            <div>
              {!isAuthorized && <span>{description} · </span>}
              <button
                type="button"
                onClick={() => setShowActions(true)}
                className="underline transition-colors hover:text-neutral-900"
              >
                Having trouble?
              </button>
            </div>
          ) : (
            <div>
              You can{" "}
              <ActionLink onClick={onRequest} disabled={isPending}>
                Request,
              </ActionLink>{" "}
              <ActionLink onClick={onReset} disabled={isPending}>
                Reset
              </ActionLink>{" "}
              or{" "}
              <ActionLink onClick={onOpen} disabled={isPending}>
                Open
              </ActionLink>{" "}
              permission panel.
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
          isAuthorized && "bg-stone-100 text-stone-800 hover:bg-stone-200",
        ])}
        aria-label={
          isAuthorized
            ? `Open ${title.toLowerCase()} settings`
            : `Request ${title.toLowerCase()} permission`
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

export function Permissions() {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const mic = usePermission("microphone");
  const systemAudio = usePermission("systemAudio");
  const accessibility = usePermission("accessibility");

  return (
    <div>
      <h2 className="mb-4 font-serif text-lg font-semibold">Permissions</h2>
      <div className="flex flex-col gap-4">
        <PermissionRow
          title="Microphone"
          description="Required to record your voice during meetings and calls"
          status={mic.status}
          isPending={mic.isPending}
          onRequest={mic.request}
          onReset={mic.reset}
          onOpen={mic.open}
        />
        <PermissionRow
          title="System audio"
          description="Required to capture other participants' voices in meetings"
          status={systemAudio.status}
          isPending={systemAudio.isPending}
          onRequest={systemAudio.request}
          onReset={systemAudio.reset}
          onOpen={systemAudio.open}
        />
        <PermissionRow
          title="Accessibility"
          description="Required to detect meeting apps and sync mute status"
          status={accessibility.status}
          isPending={accessibility.isPending}
          onRequest={accessibility.request}
          onReset={accessibility.reset}
          onOpen={accessibility.open}
        />
        {isMacos && (
          <PermissionRow
            title="Calendar"
            description="Required to sync Apple Calendar events into Char"
            status={calendar.status}
            isPending={calendar.isPending}
            onRequest={calendar.request}
            onReset={calendar.reset}
            onOpen={calendar.open}
          />
        )}
      </div>
    </div>
  );
}
