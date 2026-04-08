import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

export function MockWindow({
  showAudioIndicator,
  variant = "desktop",
  className,
  title,
  prefixIcons,
  headerClassName,
  audioIndicatorColor,
  children,
}: {
  showAudioIndicator?: boolean;
  variant?: "desktop" | "mobile";
  className?: string;
  title?: string;
  prefixIcons?: React.ReactNode;
  headerClassName?: string;
  audioIndicatorColor?: string;
  children: React.ReactNode;
}) {
  const isMobile = variant === "mobile";

  return (
    <div
      className={cn([
        "overflow-hidden border border-b-0 border-neutral-200 bg-white shadow-lg",
        isMobile ? "rounded-t-lg" : "w-full max-w-lg rounded-t-xl",
        className,
      ])}
    >
      <div
        className={cn([
          "relative flex h-[38px] items-center gap-2 border-b border-neutral-200 bg-neutral-50 px-4",
          headerClassName,
        ])}
      >
        <div className="flex gap-2">
          <div className="size-3 rounded-full bg-red-400"></div>
          <div className="size-3 rounded-full bg-yellow-400"></div>
          <div className="size-3 rounded-full bg-green-400"></div>
        </div>

        {prefixIcons && (
          <div className="ml-1 flex items-center gap-1">{prefixIcons}</div>
        )}

        {title && (
          <div className="absolute left-1/2 -translate-x-1/2">
            <span className="text-sm font-medium text-neutral-600">
              {title}
            </span>
          </div>
        )}

        {showAudioIndicator && (
          <div className="ml-auto">
            <DancingSticks
              amplitude={1}
              height={isMobile ? 10 : 12}
              color={audioIndicatorColor ?? "#a3a3a3"}
            />
          </div>
        )}
      </div>
      {children}
    </div>
  );
}
