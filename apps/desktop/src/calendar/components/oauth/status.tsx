import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

export function ReconnectRequiredIndicator() {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <span {...stylex.props(styles.indicator)} />
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <Trans>Reconnect required</Trans>
      </TooltipContent>
    </Tooltip>
  );
}

const styles = stylex.create({
  indicator: {
    backgroundColor: "rgb(245 158 11)",
    borderRadius: "9999px",
    height: "0.625rem",
    width: "0.625rem",
  },
});
