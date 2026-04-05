import { Icon } from "@iconify-icon/react";

import { cn } from "@hypr/utils";

import { usePlatform } from "@/hooks/use-platform";
import { useAnalytics } from "@/hooks/use-posthog";

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
          icon: "mdi:apple",
          label: "Download for Mac",
          href: "/download/apple-silicon",
        };
      case "windows":
        return {
          icon: "mdi:microsoft-windows",
          label: "Download Char",
          href: "/download/",
        };
      case "linux":
        return {
          icon: "mdi:apple",
          label: "Download Char",
          href: "/download/",
        };
      default:
        return {
          icon: "mdi:apple",
          label: "Download for Mac",
          href: "/download/apple-silicon",
        };
    }
  };

  const { icon, label, href } = getPlatformData();

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
        <Icon icon={icon} className="relative mr-2 mb-0.5 text-xl" />
        <span className="relative">{label}</span>
      </a>
    </div>
  );
}
