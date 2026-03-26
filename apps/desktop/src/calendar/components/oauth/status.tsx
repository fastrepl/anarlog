import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

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
