import { cn } from "@hypr/utils";

export const appFloatingContentClassName =
  "overflow-hidden rounded-2xl border border-neutral-200 bg-stone-50 p-1 shadow-lg";

export type FloatingContentVariant = "default" | "app";

export function AppFloatingPanel({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn([
        "rounded-xl border border-neutral-200 bg-white",
        className,
      ])}
      {...props}
    />
  );
}
