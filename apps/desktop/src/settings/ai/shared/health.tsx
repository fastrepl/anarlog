import { AlertCircleIcon, CheckCircleIcon, Loader2Icon } from "lucide-react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

type Props =
  | { status?: "success" | null }
  | { status: "pending" | "error"; tooltip: string };

export function ConnectionHealth(props: Props) {
  if (!props.status) {
    return null;
  }

  const color =
    props.status === "pending"
      ? "text-yellow-500"
      : props.status === "error"
        ? "text-red-500"
        : "text-green-500";

  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <div className={color}>
          {props.status === "pending" ? (
            <Loader2Icon size={16} className="animate-spin" />
          ) : props.status === "error" ? (
            <AlertCircleIcon size={16} />
          ) : props.status === "success" ? (
            <CheckCircleIcon size={16} />
          ) : null}
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-xs">
        <p className="text-xs">
          {props.status === "success"
            ? "Connection ready"
            : "tooltip" in props
              ? props.tooltip
              : null}
        </p>
      </TooltipContent>
    </Tooltip>
  );
}

export function AvailabilityHealth({ message }: { message: string }) {
  return (
    <div
      className={cn([
        "flex items-center justify-center gap-2 text-center",
        "border-b border-red-200 bg-red-50/70",
        "-mx-6 -mt-6 px-4 py-3",
        "text-sm text-red-700",
      ])}
    >
      <AlertCircleIcon className="h-4 w-4 shrink-0" />
      {message}
    </div>
  );
}
