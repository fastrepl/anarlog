import { cn } from "@hypr/utils";

import { usePlatform } from "@/hooks/use-platform";
import { useAnalytics } from "@/hooks/use-posthog";

function AppleIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className}>
      <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11" />
    </svg>
  );
}

function WindowsIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className}>
      <path d="M3 12V6.75l6-1.32v6.48L3 12zm17-9v8.75l-10 .08V5.98L20 3zm-10 9.04l10 .07V21l-10-1.39v-7.57zM3 12.25l6 .09v6.33l-6-1.07V12.25z" />
    </svg>
  );
}

function PlatformIcon({
  platform,
  className,
}: {
  platform: string;
  className?: string;
}) {
  if (platform === "windows") {
    return <WindowsIcon className={className} />;
  }
  return <AppleIcon className={className} />;
}

export function DownloadButton({
  variant = "default",
}: {
  variant?: "default" | "compact";
}) {
  const platform = usePlatform();
  const { track } = useAnalytics();

  const getPlatformData = () => {
    switch (platform) {
      case "mac":
        return {
          label: "Download for Mac",
          href: "/download/apple-silicon",
        };
      case "windows":
        return {
          label: "Download Char",
          href: "/download/",
        };
      case "linux":
        return {
          label: "Download Char",
          href: "/download/",
        };
      default:
        return {
          label: "Download for Mac",
          href: "/download/apple-silicon",
        };
    }
  };

  const { label, href } = getPlatformData();

  const handleClick = () => {
    track("download_clicked", {
      platform: platform,
      timestamp: new Date().toISOString(),
    });
  };

  if (variant === "compact") {
    return (
      <div className="rounded-full bg-gradient-to-b from-gray-100 to-gray-700 shadow-sm transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]">
        <a
          href={href}
          download
          onClick={handleClick}
          className={cn([
            "group relative flex h-9 items-center justify-center overflow-hidden px-5 text-sm",
            "surface-dark rounded-full text-white",
          ])}
        >
          <div
            className={cn([
              "pointer-events-none absolute -top-4 left-1/2 -translate-x-1/2",
              "h-9 w-full opacity-40",
            ])}
            style={{
              background:
                "radial-gradient(50% 100% at 50% 0%, white, transparent)",
            }}
          />
          <span className="relative">Download</span>
        </a>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 rounded-full bg-gradient-to-b from-gray-100 to-gray-700 shadow-md transition-all hover:scale-[102%] hover:shadow-xl active:scale-[98%]">
      <a
        href={href}
        download
        onClick={handleClick}
        className={cn([
          "group relative flex h-14 items-center justify-center overflow-hidden pr-8 pl-4",
          "surface-dark rounded-full text-white",
        ])}
      >
        <div
          className={cn([
            "pointer-events-none absolute -top-4 left-1/2 -translate-x-1/2",
            "h-14 w-full opacity-40",
          ])}
          style={{
            background:
              "radial-gradient(50% 100% at 50% 0%, white, transparent)",
          }}
        />
        <PlatformIcon
          platform={platform}
          className="relative mr-2 mb-0.5 size-5"
        />
        <span className="relative">{label}</span>
      </a>
    </div>
  );
}
