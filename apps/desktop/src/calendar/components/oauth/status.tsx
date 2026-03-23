import { ArrowLeftIcon } from "lucide-react";
import { useEffect, useState } from "react";

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

export function ConnectionTroubleShootingLink({
  connections,
}: {
  connections: ConnectionAction[];
}) {
  const [showActions, setShowActions] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [pickerPos, setPickerPos] = useState<{ x: number; y: number } | null>(
    null,
  );

  useEffect(() => {
    setShowActions(false);
    setSelectedIndex(0);
    setPickerPos(null);
  }, [connections]);

  if (connections.length === 0) return null;

  const selected = connections[selectedIndex];
  const hasMultiple = connections.length > 1;

  const handleClick = (e: React.MouseEvent) => {
    if (hasMultiple) {
      setPickerPos({ x: e.clientX, y: e.clientY });
    } else {
      setShowActions(true);
    }
  };

  const handlePickConnection = (i: number) => {
    setSelectedIndex(i);
    setPickerPos(null);
    setShowActions(true);
  };

  return (
    <div className="text-xs text-neutral-600">
      {!showActions ? (
        <>
          <button
            type="button"
            onClick={handleClick}
            className="underline transition-colors hover:text-neutral-900"
          >
            Having trouble?
          </button>
          {pickerPos && (
            <>
              <div
                className="fixed inset-0 z-50"
                onClick={() => setPickerPos(null)}
              />
              <div
                className="fixed z-50 rounded border bg-white py-1 shadow-md"
                style={{ left: pickerPos.x, top: pickerPos.y }}
              >
                {connections.map((conn, i) => (
                  <button
                    key={conn.connectionId}
                    type="button"
                    onClick={() => handlePickConnection(i)}
                    className={cn([
                      "block w-full truncate px-3 py-1.5 text-left text-xs",
                      "transition-colors hover:bg-neutral-50",
                    ])}
                  >
                    {conn.label}
                  </button>
                ))}
              </div>
            </>
          )}
        </>
      ) : (
        <div>
          <ConnectedIndicator />{" "}
          {hasMultiple && (
            <span className="font-medium text-neutral-800">
              {selected.label}
            </span>
          )}
          {hasMultiple && " — "}
          <ActionLink onClick={selected.onReconnect}>Reconnect</ActionLink> or{" "}
          <ActionLink
            onClick={selected.onDisconnect}
            className="text-red-500 hover:text-red-700"
          >
            Disconnect
          </ActionLink>
          .{" "}
          <ActionLink
            onClick={() => {
              setShowActions(false);
              setPickerPos(null);
            }}
          >
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
