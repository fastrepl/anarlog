import { ChevronUpIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

export function TodayButton({
  onClick,
  visible,
}: {
  onClick: () => void;
  visible: boolean;
}) {
  if (!visible) return null;

  return (
    <div className="absolute top-2 left-1/2 z-10 -translate-x-1/2">
      <Button
        onClick={onClick}
        size="sm"
        className={cn([
          "rounded-full bg-white hover:bg-neutral-50",
          "border border-neutral-200 text-neutral-700",
          "flex items-center gap-1",
          "shadow-xs",
        ])}
        variant="outline"
      >
        <ChevronUpIcon size={12} />
        <span className="text-xs">Go back to Today</span>
      </Button>
    </div>
  );
}
