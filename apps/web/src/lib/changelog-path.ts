export const changelogStreams = ["desktop", "mobile"] as const;
export type ChangelogStream = (typeof changelogStreams)[number];

export function isChangelogStream(value: unknown): value is ChangelogStream {
  return value === "desktop" || value === "mobile";
}

export function getChangelogReleaseFromPath(filePath: string) {
  const match = filePath
    .replaceAll("\\", "/")
    .match(/(?:^|\/)(desktop|mobile)\/(\d+\.\d+\.\d+)\.md$/);
  if (!match || !isChangelogStream(match[1])) return null;
  return { stream: match[1], version: match[2] };
}

export function changelogStreamLabel(stream: ChangelogStream) {
  return stream === "desktop" ? "Desktop" : "Mobile";
}
