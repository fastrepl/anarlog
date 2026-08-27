const latestStableDownloadUrl = "https://anarlog.so/releases/latest/platform";

function getStableDownloadUrl(platform: string) {
  return `${latestStableDownloadUrl}/${platform}?channel=stable`;
}

export type DesktopPlatform = "linux" | "macos" | "windows";

export const appleSiliconDownloadUrl = getStableDownloadUrl("dmg-aarch64");
export const appleIntelDownloadUrl = getStableDownloadUrl("dmg-x86_64");

export const comingSoonPlatforms = [
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
    platform: "windows",
    name: "Windows",
    status: "Beta",
    description: "Beta installer for 64-bit Windows PCs.",
    downloads: [
      {
        name: "Windows x64",
        detail: "NSIS installer · EXE",
        url: getStableDownloadUrl("nsis-x86_64"),
        showInMenu: true,
      },
    ],
  },
  {
    platform: "linux",
    name: "Linux",
    status: "Beta",
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
