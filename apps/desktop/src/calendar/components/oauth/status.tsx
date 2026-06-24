import { Trans } from "@lingui/react/macro";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

export function ReconnectRequiredIndicator() {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <span className="size-2.5 rounded-full bg-amber-500" />
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <Trans>Reconnect required</Trans>
      </TooltipContent>
    </Tooltip>
  );
}
