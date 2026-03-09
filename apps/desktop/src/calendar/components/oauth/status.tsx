import { ArrowLeftIcon } from "lucide-react";
import { useState } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

export function ConnectedIndicator() {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <span className="size-2.5 rounded-full bg-green-500" />
      </TooltipTrigger>
      <TooltipContent side="bottom">Connected</TooltipContent>
    </Tooltip>
  );
}

export function ReconnectRequiredIndicator() {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <span className="size-2.5 rounded-full bg-amber-500" />
      </TooltipTrigger>
      <TooltipContent side="bottom">Reconnect required</TooltipContent>
    </Tooltip>
  );
}

export function ConnectionTroubleShootingLink({
  onConnect,
  onDisconnect,
}: {
  onConnect: () => void;
  onDisconnect: () => void;
}) {
  const [showActions, setShowActions] = useState(false);

  return (
    <div className="text-xs text-neutral-600">
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
          <ConnectedIndicator />{" "}
          <ActionLink onClick={onConnect}>Reconnect</ActionLink> or{" "}
          <ActionLink
            onClick={onDisconnect}
            className="text-red-500 hover:text-red-700"
          >
            Disconnect
          </ActionLink>
          .{" "}
          <ActionLink onClick={() => setShowActions(false)}>
            <ArrowLeftIcon className="inline-block size-3" /> Back
          </ActionLink>
        </div>
      )}
    </div>
  );
}

function ActionLink({
  onClick,
  disabled,
  className,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
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
        className,
      ])}
    >
      {children}
    </button>
  );
}
