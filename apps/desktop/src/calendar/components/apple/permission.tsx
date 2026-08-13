import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  WarningCircle,
} from "@phosphor-icons/react";
import { useState } from "react";

import { type PermissionStatus } from "@anlg/plugin-permissions";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";

export function AppleCalendarPermissionDialog({
  open,
  onOpenChange,
  onOpenSettings,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onOpenSettings: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader className="items-center gap-2 text-center sm:text-center">
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            <Trans>Apple Calendar access is off</Trans>
          </DialogTitle>
          <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
            <Trans>
              Turn on Anarlog in System Settings → Privacy &amp; Security →
              Calendars, then return here.
            </Trans>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 sm:grid-cols-2 sm:justify-normal">
          <GlassDialogCancelButton onClick={() => onOpenChange(false)}>
            <Trans>Cancel</Trans>
          </GlassDialogCancelButton>
          <Button
            className="bg-primary text-primary-foreground hover:bg-primary/90 h-8 rounded-full px-4 text-xs font-medium shadow-sm dark:bg-white dark:text-black dark:hover:bg-white/90"
            onClick={() => {
              onOpenSettings();
              onOpenChange(false);
            }}
          >
            <Trans>Open Settings</Trans>
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
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
          {!isAuthorized && <WarningCircle className="size-4" />}
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
            <Check className="size-5" />
          ) : (
            <ArrowRight className="size-5" />
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
            <ArrowLeft className="inline-block size-3 underline" />
            <Trans>Back</Trans>
          </ActionLink>
        </div>
      )}
    </div>
  );
}
