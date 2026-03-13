import type { InstalledApp } from "@hypr/plugin-detect";

export function getIgnoredAppOptions({
  allInstalledApps,
  ignoredPlatforms,
  inputValue,
  defaultIgnoredBundleIds,
}: {
  allInstalledApps: InstalledApp[] | undefined;
  ignoredPlatforms: string[];
  inputValue: string;
  defaultIgnoredBundleIds: string[] | undefined;
}) {
  return (allInstalledApps ?? []).filter((app) => {
    const matchesSearch = app.name
      .toLowerCase()
      .includes(inputValue.toLowerCase());
    const notAlreadyAdded = !ignoredPlatforms.includes(app.id);
    const notDefaultIgnored = !(defaultIgnoredBundleIds ?? []).includes(app.id);
    return matchesSearch && notAlreadyAdded && notDefaultIgnored;
  });
}
