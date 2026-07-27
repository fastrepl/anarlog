const latestStableDownloadUrl =
  "https://cdn.crabnebula.app/download/fastrepl/hyprnote2/latest/platform";

function getStableDownloadUrl(platform: string) {
  return `${latestStableDownloadUrl}/${platform}?channel=stable`;
}

export const appleSiliconDownloadUrl = getStableDownloadUrl("dmg-aarch64");
export const appleIntelDownloadUrl = getStableDownloadUrl("dmg-x86_64");

export const desktopDownloadSections = [
  {
    name: "macOS",
    description: "Choose the build that matches your Mac.",
    downloads: [
      {
        name: "Apple Silicon",
        detail: "M-series Mac · DMG",
        url: appleSiliconDownloadUrl,
      },
      {
        name: "Intel",
        detail: "Intel-based Mac · DMG",
        url: appleIntelDownloadUrl,
      },
    ],
  },
  {
    name: "Windows",
    description:
      "Preview installer for 64-bit Windows PCs. Physical audio and AEC validation is pending.",
    downloads: [
      {
        name: "Windows x64",
        detail: "NSIS installer · EXE",
        url: getStableDownloadUrl("nsis-x86_64"),
      },
    ],
  },
  {
    name: "Linux",
    description:
      "Beta AppImage and Debian packages for x64 and ARM64. Physical audio and AEC validation is pending.",
    downloads: [
      {
        name: "AppImage x64",
        detail: "Intel or AMD 64-bit · AppImage",
        url: getStableDownloadUrl("appimage-x86_64"),
      },
      {
        name: "Debian x64",
        detail: "Debian or Ubuntu · DEB",
        url: getStableDownloadUrl("deb-x86_64"),
      },
      {
        name: "AppImage ARM64",
        detail: "64-bit ARM · AppImage",
        url: getStableDownloadUrl("appimage-aarch64"),
      },
      {
        name: "Debian ARM64",
        detail: "Debian or Ubuntu on 64-bit ARM · DEB",
        url: getStableDownloadUrl("deb-aarch64"),
      },
    ],
  },
] as const;
