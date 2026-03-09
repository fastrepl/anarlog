import { WrenchIcon } from "lucide-react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

export type McpIndicator = { type: "support" };

export function McpIndicatorBadge({ indicator }: { indicator: McpIndicator }) {
  if (indicator.type === "support") {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="flex items-center gap-1 text-neutral-400">
            <WrenchIcon size={12} />
            <span className="text-[11px] leading-none">Support MCP</span>
          </div>
        </TooltipTrigger>
        <TooltipContent className="z-110">
          Connected to support tools
        </TooltipContent>
      </Tooltip>
    );
  }
}
