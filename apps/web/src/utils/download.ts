export type DownloadPlatform = "macos";
export type DownloadArch = "aarch64" | "x86_64";
export type DownloadFormat = "dmg";

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
  ];
}

export function groupDownloadLinks(links: DownloadLink[]) {
  return {
    macos: links.filter((link) => link.platform === "macos"),
  };
}
