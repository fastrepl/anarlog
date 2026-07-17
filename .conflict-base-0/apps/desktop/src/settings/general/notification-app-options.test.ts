import { describe, expect, test } from "vitest";

import {
  getIgnoredBundleIds,
  getIgnorableApps,
} from "./notification-app-options";

describe("getIgnorableApps", () => {
  test("hides default ignored apps from dropdown matches", () => {
    const options = getIgnorableApps({
      installedApps: [{ id: "com.microsoft.VSCode", name: "VS Code" }],
      ignoredPlatforms: [],
      includedPlatforms: [],
      inputValue: "code",
      defaultIgnoredBundleIds: ["com.microsoft.VSCode"],
    });

    expect(options).toEqual([]);
  });

  test("shows explicitly included default apps as excludable again", () => {
    const options = getIgnorableApps({
      installedApps: [{ id: "com.microsoft.VSCode", name: "VS Code" }],
      ignoredPlatforms: [],
      includedPlatforms: ["com.microsoft.VSCode"],
      inputValue: "code",
      defaultIgnoredBundleIds: ["com.microsoft.VSCode"],
    });

    expect(options).toEqual([{ id: "com.microsoft.VSCode", name: "VS Code" }]);
  });
});

describe("getIgnoredBundleIds", () => {
  test("includes installed default ignored apps unless explicitly included", () => {
    expect(
      getIgnoredBundleIds({
        installedApps: [
          { id: "com.microsoft.VSCode", name: "VS Code" },
          { id: "us.zoom.xos", name: "Zoom Workplace" },
        ],
        ignoredPlatforms: [],
        includedPlatforms: [],
        defaultIgnoredBundleIds: ["com.microsoft.VSCode"],
      }),
    ).toEqual(["com.microsoft.VSCode"]);

    expect(
      getIgnoredBundleIds({
        installedApps: [{ id: "com.microsoft.VSCode", name: "VS Code" }],
        ignoredPlatforms: [],
        includedPlatforms: ["com.microsoft.VSCode"],
        defaultIgnoredBundleIds: ["com.microsoft.VSCode"],
      }),
    ).toEqual([]);
  });
});
