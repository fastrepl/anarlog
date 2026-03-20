import { cn } from "@hypr/utils";

function Kbd({ className, ...props }: React.ComponentProps<"kbd">) {
  return (
    <kbd
      data-slot="kbd"
      className={cn([
        "pointer-events-none inline-flex h-5 w-fit min-w-5 items-center justify-center gap-1 rounded px-1 font-mono text-xs font-medium select-none",
        "border border-neutral-300",
        "bg-linear-to-b from-white to-neutral-100",
        "text-neutral-400",
        "shadow-[0_1px_0_0_rgba(0,0,0,0.1),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
        "[&_svg:not([class*='size-'])]:size-3",
        className,
      ])}
      {...props}
    />
  );
}

function KbdGroup({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="kbd-group"
      className={cn(["inline-flex items-center gap-0.5", className])}
      {...props}
    />
  );
}

export { Kbd, KbdGroup };
