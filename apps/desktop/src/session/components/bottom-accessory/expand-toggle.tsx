import { X } from "lucide-react";

import { cn } from "@hypr/utils";

export function ExpandToggle({
  isExpanded,
  onToggle,
  label,
  showExpandedCloseIcon = false,
  collapsedClassName,
}: {
  isExpanded: boolean;
  onToggle: () => void;
  label?: string;
  showExpandedCloseIcon?: boolean;
  collapsedClassName?: string;
}) {
  const hasLabel = Boolean(label);

  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn([
        "absolute left-3 z-10",
        "relative flex h-5 items-center justify-center gap-1",
        hasLabel ? "px-3" : "w-10",
        "rounded-t-[10px] rounded-b-none border-x border-t border-neutral-200",
        "text-neutral-400",
        isExpanded ? "bg-white" : (collapsedClassName ?? "bg-white"),
        "transition-colors hover:bg-neutral-100 hover:text-neutral-600",
        "hover:cursor-pointer",
      ])}
      aria-label={
        isExpanded ? `Collapse ${label ?? ""}`.trim() : `Expand ${label ?? ""}`
      }
    >
      {label ? <span className="text-[10px] font-medium">{label}</span> : null}
      {isExpanded && showExpandedCloseIcon ? (
        <X size={10} className="shrink-0" />
      ) : null}
    </button>
  );
}
