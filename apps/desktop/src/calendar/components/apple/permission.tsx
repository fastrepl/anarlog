import {
  AlertCircleIcon,
  ArrowLeftIcon,
  ArrowRightIcon,
  CheckIcon,
} from "lucide-react";
import { useState } from "react";

import { type PermissionStatus } from "@hypr/plugin-permissions";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { usePermission } from "~/shared/hooks/usePermissions";

export function ApplePermissions() {
  const calendar = usePermission("calendar");

  return (
    <div className="flex flex-col gap-1">
      <AccessPermissionRow
        title="Apple Calendar"
        status={calendar.status}
        isPending={calendar.isPending}
        onOpen={calendar.open}
        onRequest={calendar.request}
        onReset={calendar.reset}
        showActionButton={false}
      />
    </div>
  );
}

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

export function AccessPermissionRow({
  title,
  status,
  isPending,
  onOpen,
  onRequest,
  onReset,
  showActionButton = true,
}: {
  title: string;
  status: PermissionStatus | undefined;
  isPending: boolean;
  onOpen: () => void;
  onRequest: () => void;
  onReset: () => void;
  showActionButton?: boolean;
}) {
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
    <div
      className={cn([
        "flex gap-4 py-2",
        showActionButton
          ? "items-center justify-between"
          : "items-start justify-start",
      ])}
    >
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
        <TroubleShootingLink
          onRequest={onRequest}
          onReset={onReset}
          onOpen={onOpen}
          isPending={isPending}
        />
      </div>
      {showActionButton && (
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
              : `Request ${title.toLowerCase()}`
          }
        >
          {isAuthorized ? (
            <CheckIcon className="size-5" />
          ) : (
            <ArrowRightIcon className="size-5" />
          )}
        </Button>
      )}
    </div>
  );
}

export function TroubleShootingLink({
  onRequest,
  onReset,
  onOpen,
  isPending,
  className,
}: {
  onRequest: () => void;
  onReset: () => void;
  onOpen: () => void;
  isPending: boolean;
  className?: string;
}) {
  const [showActions, setShowActions] = useState(false);
  return (
    <div className={cn(["text-xs text-neutral-600", className])}>
      {!showActions ? (
        <button
          type="button"
          onClick={() => setShowActions(true)}
          className="underline transition-colors hover:text-neutral-900"
        >
          Having trouble?
        </button>
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
          permission panel.{" "}
          <ActionLink onClick={() => setShowActions(false)}>
            <ArrowLeftIcon className="inline-block size-3 underline" />
            Back
          </ActionLink>
        </div>
      )}
    </div>
  );
}
