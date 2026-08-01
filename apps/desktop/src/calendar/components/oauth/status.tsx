import { Trans } from "@lingui/react/macro";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

export function ReconnectRequiredIndicator() {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <span className="shape-circle size-2.5 rounded-full bg-amber-500" />
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <Trans>Reconnect required</Trans>
      </TooltipContent>
    </Tooltip>
  );
}
