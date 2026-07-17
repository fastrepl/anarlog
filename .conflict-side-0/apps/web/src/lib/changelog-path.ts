const CHANGELOG_VERSION_FILE_PATTERN = /(?:^|\/)(\d+\.\d+\.\d+)\.md$/;

export function getChangelogVersionFromPath(filePath: string) {
  return filePath.match(CHANGELOG_VERSION_FILE_PATTERN)?.[1] ?? null;
}
