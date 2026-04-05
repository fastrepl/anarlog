import type { InstalledApp } from "@hypr/plugin-detect";

function isAppIgnored({
  bundleId,
  ignoredPlatforms,
  includedPlatforms,
  defaultIgnoredBundleIds,
}: {
  bundleId: string;
  ignoredPlatforms: string[];
  includedPlatforms: string[];
  defaultIgnoredBundleIds: string[];
}) {
  const isDefaultIgnored = defaultIgnoredBundleIds.includes(bundleId);
  const isIncluded = includedPlatforms.includes(bundleId);
  const isUserIgnored = ignoredPlatforms.includes(bundleId);

  return isUserIgnored || (isDefaultIgnored && !isIncluded);
}

// Installed apps that aren't already ignored and match the search query.
export function getIgnorableApps({
  installedApps,
  ignoredPlatforms,
  includedPlatforms,
  inputValue,
  defaultIgnoredBundleIds,
}: {
  installedApps: InstalledApp[];
  ignoredPlatforms: string[];
  includedPlatforms: string[];
  inputValue: string;
  defaultIgnoredBundleIds: string[];
}) {
  return installedApps.filter((app) => {
    const matchesSearch = app.name
      .toLowerCase()
      .includes(inputValue.trim().toLowerCase());
    const isIgnored = isAppIgnored({
      bundleId: app.id,
      ignoredPlatforms,
      includedPlatforms,
      defaultIgnoredBundleIds,
    });
    return matchesSearch && !isIgnored;
  });
}

export function getIgnoredBundleIds({
  installedApps,
  ignoredPlatforms,
  includedPlatforms,
  defaultIgnoredBundleIds,
}: {
  installedApps: InstalledApp[];
  ignoredPlatforms: string[];
  includedPlatforms: string[];
  defaultIgnoredBundleIds: string[];
}) {
  const installedAppIds = new Set(installedApps.map((app) => app.id));
  const bundleIds = new Set(ignoredPlatforms);

  for (const bundleId of defaultIgnoredBundleIds) {
    if (!installedAppIds.has(bundleId)) {
      continue;
    }

    if (
      isAppIgnored({
        bundleId,
        ignoredPlatforms,
        includedPlatforms,
        defaultIgnoredBundleIds,
      })
    ) {
      bundleIds.add(bundleId);
    }
  }

  return [...bundleIds];
}

export function toggleIgnoredApp({
  bundleId,
  ignoredPlatforms,
  includedPlatforms,
  defaultIgnoredBundleIds,
}: {
  bundleId: string;
  ignoredPlatforms: string[];
  includedPlatforms: string[];
  defaultIgnoredBundleIds: string[];
}) {
  const isIgnored = isAppIgnored({
    bundleId,
    ignoredPlatforms,
    includedPlatforms,
    defaultIgnoredBundleIds,
  });
  const isIgnoredByDefault = defaultIgnoredBundleIds.includes(bundleId);
  let newIgnoredPlatforms: string[];
  let newIncludedPlatforms: string[];
  if (isIgnored) {
    // if ignored, remove from ignoredPlatforms
    // additionally, if bundleId is also ignored by default, add it to includedPlatforms as well
    newIgnoredPlatforms = ignoredPlatforms.filter((id) => id !== bundleId);
    newIncludedPlatforms = isIgnoredByDefault
      ? [...includedPlatforms, bundleId]
      : includedPlatforms;
  } else {
    // if not ignored *and* not a ignored-by-default app, add it to ignoredPlatforms
    // also remove from includedPlatforms if exists
    newIgnoredPlatforms = isIgnoredByDefault
      ? ignoredPlatforms
      : [...ignoredPlatforms, bundleId];
    newIncludedPlatforms = includedPlatforms.filter((id) => id !== bundleId);
  }

  return {
    ignoredPlatforms: newIgnoredPlatforms,
    includedPlatforms: newIncludedPlatforms,
  };
}
