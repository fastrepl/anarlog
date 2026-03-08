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
          label: "Download for Windows",
          href: "/download/windows",
        };
      case "linux":
        return {
          icon: "mdi:linux",
          label: "Download for Linux",
          href: "/download/linux",
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
    <a
      href={href}
      download
      onClick={handleClick}
      className={cn([
        "group flex h-12 items-center justify-center px-6",
        "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
        "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
        "transition-all",
      ])}
    >
      <Icon icon={icon} className="mr-2 text-xl" />
      {label}
    </a>
  );
}
