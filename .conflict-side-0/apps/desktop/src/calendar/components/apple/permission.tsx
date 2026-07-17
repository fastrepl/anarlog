import { Trans, useLingui } from "@lingui/react/macro";
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
  const { t } = useLingui();
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
            isAuthorized && "bg-muted text-foreground hover:bg-accent",
          ])}
          aria-label={
            isAuthorized
              ? t`Open ${title.toLowerCase()} settings`
              : t`Request ${title.toLowerCase()}`
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
  const { t } = useLingui();
  const [showActions, setShowActions] = useState(false);
  return (
    <div className={cn(["text-muted-foreground text-xs", className])}>
      {!showActions ? (
        <button
          type="button"
          onClick={() => setShowActions(true)}
          className="hover:text-foreground underline transition-colors"
        >
          <Trans>Having trouble?</Trans>
        </button>
      ) : (
        <div>
          <Trans>You can</Trans>{" "}
          <ActionLink onClick={onRequest} disabled={isPending}>
            {t`Request`},
          </ActionLink>{" "}
          <ActionLink onClick={onReset} disabled={isPending}>
            <Trans>Reset</Trans>
          </ActionLink>{" "}
          <Trans>or</Trans>{" "}
          <ActionLink onClick={onOpen} disabled={isPending}>
            <Trans>Open</Trans>
          </ActionLink>{" "}
          <Trans>permission panel.</Trans>{" "}
          <ActionLink onClick={() => setShowActions(false)}>
            <ArrowLeftIcon className="inline-block size-3 underline" />
            <Trans>Back</Trans>
          </ActionLink>
        </div>
      )}
    </div>
  );
}
