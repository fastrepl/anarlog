import { cn } from "@hypr/utils";

export function MenuItem({
  icon: Icon,
  label,
  badge,
  suffixIcon: SuffixIcon,
  onClick,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  badge?: number | React.ReactNode;
  suffixIcon?: React.ComponentType<{ className?: string }>;
  onClick: () => void;
}) {
  return (
    <div className="px-1">
      <button
        className={cn([
          "group flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg",
          "px-3 py-1.5",
          "text-sm whitespace-nowrap text-black",
          "transition-colors hover:bg-neutral-100",
        ])}
        onClick={onClick}
      >
        <div className="flex items-center justify-start gap-2.5">
          <Icon className="h-4 w-4 shrink-0 text-black" />
          {label}
        </div>
        {badge &&
          (typeof badge === "number" ? (
            <span
              className={cn(
                "rounded-full",
                "px-2 py-0.5",
                "bg-red-500",
                "text-xs font-semibold text-white",
              )}
            >
              {badge}
            </span>
          ) : (
            badge
          ))}
        {SuffixIcon && (
          <SuffixIcon className={cn("h-4 w-4 shrink-0", "text-neutral-400")} />
        )}
      </button>
    </div>
  );
}
