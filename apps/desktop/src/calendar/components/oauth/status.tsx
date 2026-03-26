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

export interface ConnectionAction {
  connectionId: string;
  label: string;
  onReconnect: () => void;
  onDisconnect: () => void;
}

export function ConnectionActionList({
  connections,
}: {
  connections: ConnectionAction[];
}) {
  if (connections.length === 0) return null;

  return (
    <div className="flex flex-col gap-1 text-xs text-neutral-600">
      {connections.map((connection) => (
        <div key={connection.connectionId} className="flex flex-wrap gap-x-1.5">
          <span className="font-medium text-neutral-800">
            {connection.label}
          </span>
          <ActionLink onClick={connection.onReconnect}>Reconnect</ActionLink>
          <span className="text-neutral-400">or</span>
          <ActionLink
            onClick={connection.onDisconnect}
            className="text-red-500 hover:text-red-700"
          >
            Disconnect
          </ActionLink>
        </div>
      ))}
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
