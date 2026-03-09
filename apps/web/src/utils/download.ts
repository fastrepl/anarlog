export type DownloadPlatform = "macos" | "linux";
export type DownloadArch = "aarch64" | "x86_64";
export type DownloadFormat = "dmg" | "appimage" | "deb";

export interface DownloadLink {
  platform: DownloadPlatform;
  arch: DownloadArch;
  format: DownloadFormat;
  url: string;
  label: string;
}

export function getDownloadLinks(version: string): DownloadLink[] {
  const baseUrl = `https://github.com/fastrepl/char/releases/download/desktop_v${version}`;

  return [
    {
      platform: "macos",
      arch: "aarch64",
      format: "dmg",
      url: `${baseUrl}/char-macos-aarch64.dmg`,
      label: "Apple Silicon",
    },
    {
      platform: "macos",
      arch: "x86_64",
      format: "dmg",
      url: `${baseUrl}/char-macos-x86_64.dmg`,
      label: "Intel",
    },
    {
      platform: "linux",
      arch: "x86_64",
      format: "appimage",
      url: `${baseUrl}/char-linux-x86_64.AppImage`,
      label: "AppImage (x86)",
    },
    {
      platform: "linux",
      arch: "x86_64",
      format: "deb",
      url: `${baseUrl}/char-linux-x86_64.deb`,
      label: "Debian (x86)",
    },
    {
      platform: "linux",
      arch: "aarch64",
      format: "appimage",
      url: `${baseUrl}/char-linux-aarch64.AppImage`,
      label: "AppImage (ARM)",
    },
    {
      platform: "linux",
      arch: "aarch64",
      format: "deb",
      url: `${baseUrl}/char-linux-aarch64.deb`,
      label: "Debian (ARM)",
    },
  ];
}

export function groupDownloadLinks(links: DownloadLink[]) {
  return {
    macos: links.filter((link) => link.platform === "macos"),
    linux: links.filter((link) => link.platform === "linux"),
  };
}
