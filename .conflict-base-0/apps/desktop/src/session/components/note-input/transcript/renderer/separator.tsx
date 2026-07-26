import { cn } from "@hypr/utils";

export function TranscriptSeparator() {
  return (
    <div
      className={cn([
        "flex items-center gap-3",
        "text-muted-foreground text-xs font-light",
      ])}
    >
      <div className="border-border/40 flex-1 border-t" />
      <span>~ ~ ~ ~ ~ ~ ~ ~ ~</span>
      <div className="border-border/40 flex-1 border-t" />
    </div>
  );
}
