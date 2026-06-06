import { X } from "lucide-react";

import { cn } from "@hypr/utils";

export function ExpandToggle({
  isExpanded,
  onToggle,
  label,
  showExpandedCloseIcon = false,
  collapsedClassName,
  expandedClassName,
}: {
  isExpanded: boolean;
  onToggle: () => void;
  label?: string;
  showExpandedCloseIcon?: boolean;
  collapsedClassName?: string;
  expandedClassName?: string;
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
        "border-border rounded-t-[10px] rounded-b-none border-x border-t",
        "after:pointer-events-none after:absolute after:right-px after:-bottom-px after:left-px after:h-0.5 after:bg-inherit after:content-['']",
        "text-muted-foreground",
        isExpanded
          ? (expandedClassName ?? "bg-card")
          : (collapsedClassName ?? "bg-card"),
        "hover:bg-accent hover:text-muted-foreground transition-colors",
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
