const latestStableDownloadUrl =
  "https://desktop.anarlog.so/download/latest/platform";

function getStableDownloadUrl(platform: string) {
  return `${latestStableDownloadUrl}/${platform}?channel=stable`;
}

export type DownloadPlatform =
  | "android"
  | "ios"
  | "linux"
  | "macos"
  | "windows";

export const appleSiliconDownloadUrl = getStableDownloadUrl("dmg-aarch64");
export const appleIntelDownloadUrl = getStableDownloadUrl("dmg-x86_64");
export const windowsStoreDownloadUrl =
  "https://apps.microsoft.com/detail/XPDLN196NKSW10";

export const comingSoonPlatforms = ["Apple Watch", "Galaxy Watch"] as const;

export const platformIcons = {
  macOS: "simple-icons:apple",
  Windows: "simple-icons:windows",
  Linux: "simple-icons:linux",
  iOS: "simple-icons:apple",
  Android: "simple-icons:android",
} as const;

export const mobileDownloadSections = [
  {
    platform: "ios",
    name: "iOS",
    status: "Beta",
    description: "Join the public beta on your iPhone or iPad with TestFlight.",
    downloads: [
      {
        name: "TestFlight",
        detail: "iPhone and iPad · Public beta",
        url: "https://testflight.apple.com/join/y7WJCXvG",
        actionLabel: "Join TestFlight",
        showInMenu: true,
      },
    ],
  },
  {
    platform: "android",
    name: "Android",
    status: "Beta",
    description: "Join the open beta on Google Play.",
    downloads: [
      {
        name: "Google Play",
        detail: "Android · Open beta",
        url: "https://play.google.com/apps/testing/so.anarlog.mobile",
        actionLabel: "Join open beta",
        showInMenu: true,
      },
    ],
  },
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
    platform: "windows",
    name: "Windows",
    status: null,
    description: "Choose a direct download or install from Microsoft Store.",
    downloads: [
      {
        name: "Windows x64",
        detail: "Signed EXE installer",
        url: getStableDownloadUrl("nsis-x86_64"),
        showInMenu: true,
      },
      {
        name: "Microsoft Store",
        detail: "Windows 10 and 11",
        url: windowsStoreDownloadUrl,
        actionLabel: "Get from Store",
        showInMenu: true,
      },
    ],
  },
  {
    platform: "linux",
    name: "Linux",
    status: null,
    description:
      "APT, AppImage, and Debian packages for x64 and ARM64, plus a PKGBUILD for Arch.",
    downloads: [
      {
        name: "APT repository",
        detail: "Debian or Ubuntu · Automatic updates",
        url: "https://docs.anarlog.so/desktop-installation#apt-repository",
        actionLabel: "Install with APT",
        showInMenu: false,
      },
      {
        name: "AppImage x64",
        detail: "Intel or AMD 64-bit · AppImage",
        url: getStableDownloadUrl("appimage-x86_64"),
        showInMenu: true,
      },
      {
        name: "Debian x64",
        detail: "Debian or Ubuntu · DEB",
        url: getStableDownloadUrl("debian-x86_64"),
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
        url: getStableDownloadUrl("debian-aarch64"),
        showInMenu: false,
      },
      {
        name: "Arch Linux",
        detail: "Arch-based distros · PKGBUILD",
        url: "https://github.com/fastrepl/anarlog/tree/main/packaging/aur/anarlog-bin",
        actionLabel: "View PKGBUILD",
        showInMenu: false,
      },
    ],
  },
] as const;

export function detectDownloadPlatform(
  userAgent: string,
  maxTouchPoints = 0,
): DownloadPlatform {
  if (/Android/i.test(userAgent)) return "android";
  if (
    /iPhone|iPad|iPod/i.test(userAgent) ||
    (/Macintosh/i.test(userAgent) && maxTouchPoints > 1)
  ) {
    return "ios";
  }
  if (/Windows/i.test(userAgent)) return "windows";
  if (/Macintosh|Mac OS X/i.test(userAgent)) return "macos";
  if (!/Android|CrOS/i.test(userAgent) && /Linux|X11/i.test(userAgent)) {
    return "linux";
  }

  return "macos";
}

export function getOrderedDownloadSections(
  preferredPlatform: DownloadPlatform,
) {
  const sections = [...desktopDownloadSections, ...mobileDownloadSections];
  return [
    ...sections.filter((section) => section.platform === preferredPlatform),
    ...sections.filter((section) => section.platform !== preferredPlatform),
  ];
}
