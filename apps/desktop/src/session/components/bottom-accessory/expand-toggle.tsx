import { ChevronDown, ChevronUp } from "lucide-react";

import { cn } from "@hypr/utils";

export function ExpandToggle({
  isExpanded,
  onToggle,
  label,
}: {
  isExpanded: boolean;
  onToggle: () => void;
  label?: string;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn([
        "absolute top-0 left-1/2 z-10 -translate-x-1/2 -translate-y-1/2",
        "flex h-6 items-center justify-center gap-1 rounded-full",
        label && !isExpanded ? "px-3" : "w-10",
        "border border-neutral-200 bg-white text-neutral-400 shadow-xs",
        "transition-colors hover:bg-neutral-50 hover:text-neutral-600",
      ])}
      aria-label={isExpanded ? "Collapse" : `Expand ${label ?? ""}`}
    >
      {isExpanded ? (
        <ChevronDown size={14} />
      ) : (
        <>
          <ChevronUp size={14} />
          {label && <span className="text-[10px] font-medium">{label}</span>}
        </>
      )}
    </button>
  );
}
