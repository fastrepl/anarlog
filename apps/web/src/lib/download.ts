const latestStableDownloadUrl =
  "https://cdn.crabnebula.app/download/fastrepl/hyprnote2/latest/platform";

function getStableDownloadUrl(platform: string) {
  return `${latestStableDownloadUrl}/${platform}?channel=stable`;
}

export type DesktopPlatform = "linux" | "macos" | "windows";

export const appleSiliconDownloadUrl = getStableDownloadUrl("dmg-aarch64");
export const appleIntelDownloadUrl = getStableDownloadUrl("dmg-x86_64");

export const comingSoonPlatforms = [
  "Linux",
  "Windows",
  "iOS",
  "Android",
  "Apple Watch",
  "Galaxy Watch",
] as const;

export const desktopDownloadSections = [
  {
    platform: "macos",
    name: "macOS",
    status: null,
    description: "Choose the build that matches your Mac.",
    downloads: [
      {
        name: "Apple Silicon",
        detail: "M-series Mac · DMG",
        url: appleSiliconDownloadUrl,
        showInMenu: true,
      },
      {
        name: "Intel",
        detail: "Intel-based Mac · DMG",
        url: appleIntelDownloadUrl,
        showInMenu: true,
      },
    ],
  },
] as const;

export function detectDesktopPlatform(userAgent: string): DesktopPlatform {
  if (/Windows/i.test(userAgent)) return "windows";
  if (/Macintosh|Mac OS X|iPhone|iPad|iPod/i.test(userAgent)) return "macos";
  if (!/Android|CrOS/i.test(userAgent) && /Linux|X11/i.test(userAgent)) {
    return "linux";
  }

  return "macos";
}

export function getOrderedDesktopDownloadSections(
  preferredPlatform: DesktopPlatform,
) {
  return [
    ...desktopDownloadSections.filter(
      (section) => section.platform === preferredPlatform,
    ),
    ...desktopDownloadSections.filter(
      (section) => section.platform !== preferredPlatform,
    ),
  ];
}
