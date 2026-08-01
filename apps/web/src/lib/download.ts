const latestStableDownloadUrl =
  "https://cdn.crabnebula.app/download/fastrepl/hyprnote2/latest/platform";

function getStableDownloadUrl(platform: string) {
  return `${latestStableDownloadUrl}/${platform}?channel=stable`;
}

export type DesktopPlatform = "linux" | "macos" | "windows";

export const appleSiliconDownloadUrl = getStableDownloadUrl("dmg-aarch64");
export const appleIntelDownloadUrl = getStableDownloadUrl("dmg-x86_64");

export const comingSoonPlatforms = [
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
  {
    platform: "linux",
    name: "Linux",
    status: "Beta",
    description:
      "Beta AppImage and Debian packages for x64 and ARM64. Physical audio and AEC validation is pending.",
    downloads: [
      {
        name: "AppImage x64",
        detail: "Intel or AMD 64-bit · AppImage",
        url: getStableDownloadUrl("appimage-x86_64"),
        showInMenu: true,
      },
      {
        name: "Debian x64",
        detail: "Debian or Ubuntu · DEB",
        url: getStableDownloadUrl("deb-x86_64"),
        showInMenu: true,
      },
      {
        name: "AppImage ARM64",
        detail: "64-bit ARM · AppImage",
        url: getStableDownloadUrl("appimage-aarch64"),
        showInMenu: false,
      },
      {
        name: "Debian ARM64",
        detail: "Debian or Ubuntu on 64-bit ARM · DEB",
        url: getStableDownloadUrl("deb-aarch64"),
        showInMenu: false,
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
