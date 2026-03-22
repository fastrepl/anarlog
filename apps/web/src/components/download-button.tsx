import { Icon } from "@iconify-icon/react";

import { cn } from "@hypr/utils";

import { usePlatform } from "@/hooks/use-platform";
import { useAnalytics } from "@/hooks/use-posthog";

export function DownloadButton() {
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

  return (
    <div className="flex flex-col gap-2 rounded-full bg-gradient-to-b from-gray-100 to-gray-700 p-0.5 shadow-md transition-all hover:scale-[102%] hover:shadow-xl active:scale-[98%]">
      <a
        href={href}
        download
        onClick={handleClick}
        className={cn([
          "group flex h-14 items-center justify-center px-8",
          "surface-dark rounded-full text-white",
        ])}
      >
        <Icon icon={icon} className="mr-2 mb-0.5 text-xl" />
        {label}
      </a>
    </div>
  );
}
