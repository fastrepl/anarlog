import { cn } from "@hypr/utils";

export function TranscriptSeparator() {
  return (
    <div
      className={cn([
        "flex items-center gap-3",
        "text-xs font-light text-neutral-400",
      ])}
    >
      <div className="flex-1 border-t border-neutral-200/40" />
      <span>~ ~ ~ ~ ~ ~ ~ ~ ~</span>
      <div className="flex-1 border-t border-neutral-200/40" />
    </div>
  );
}
