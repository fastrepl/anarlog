import { describe, expect, test } from "vitest";

import { getIgnoredAppOptions } from "./notification-app-options";

describe("getIgnoredAppOptions", () => {
  test("returns installed app matches for partial searches", () => {
    const options = getIgnoredAppOptions({
      allInstalledApps: [
        { id: "us.zoom.xos", name: "Zoom Workplace" },
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
      ],
      ignoredPlatforms: [],
      inputValue: "zoom",
      defaultIgnoredBundleIds: [],
    });

    expect(options).toEqual([{ id: "us.zoom.xos", name: "Zoom Workplace" }]);
  });

  test("filters out already ignored and default ignored apps", () => {
    const options = getIgnoredAppOptions({
      allInstalledApps: [
        { id: "us.zoom.xos", name: "Zoom Workplace" },
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
        { id: "com.openai.chat", name: "ChatGPT" },
      ],
      ignoredPlatforms: ["com.tinyspeck.slackmacgap"],
      inputValue: "",
      defaultIgnoredBundleIds: ["com.openai.chat"],
    });

    expect(options).toEqual([{ id: "us.zoom.xos", name: "Zoom Workplace" }]);
  });
});
