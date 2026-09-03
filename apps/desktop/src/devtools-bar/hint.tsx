import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";
import { cn } from "@anlg/utils";

/** Dark monospace tooltip shared by the bar's segments and menu items. */
export function Hint(props: {
  children: React.ReactNode;
  content: React.ReactNode;
  side?: "top" | "right";
}) {
  return (
    <Tooltip delayDuration={200}>
      <TooltipTrigger asChild>{props.children}</TooltipTrigger>
      <TooltipContent
        side={props.side ?? "top"}
        sideOffset={props.side === "right" ? 8 : 6}
        className={cn([
          "border-neutral-700 bg-neutral-900/95 text-neutral-200 shadow-xl backdrop-blur",
          "max-w-72 px-2.5 py-1.5 font-mono text-[11px] leading-4",
        ])}
      >
        {props.content}
      </TooltipContent>
    </Tooltip>
  );
}
