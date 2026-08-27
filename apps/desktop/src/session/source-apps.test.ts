import { describe, expect, it } from "vitest";

import {
  formatSessionSourceAppsContext,
  mergeSessionSourceApps,
  parseSessionSourceApps,
} from "./source-apps";

describe("session source apps", () => {
  it("preserves a raw app id when captured platform metadata uses an alias", () => {
    expect(
      mergeSessionSourceApps(
        [{ app: "chrome", name: "Google Chrome" }],
        [
          {
            app: "com.google.Chrome",
            name: "Google Chrome",
            platform: "Google Meet",
          },
        ],
      ),
    ).toEqual([
      {
        app: "chrome",
        name: "Google Chrome",
        platform: "Google Meet",
      },
    ]);
  });

  it("merges later mic apps without dropping earlier meeting sources", () => {
    expect(
      mergeSessionSourceApps(
        [{ app: "us.zoom.xos", name: "Zoom", platform: "Zoom" }],
        [{ app: "slack", name: "Slack.exe", platform: "Slack" }],
      ),
    ).toEqual([
      { app: "us.zoom.xos", name: "Zoom", platform: "Zoom" },
      { app: "slack", name: "Slack.exe", platform: "Slack" },
    ]);
  });

  it("accepts legacy app-only records and formats normalized platforms", () => {
    const parsed = parseSessionSourceApps(
      '[{"app":"zoom"},{"app":"slack","platform":"Slack"}]',
    );

    expect(parsed).toEqual([
      { app: "zoom" },
      { app: "slack", platform: "Slack" },
    ]);
    expect(formatSessionSourceAppsContext(parsed)).toBe(
      "Meeting platform: Slack",
    );
  });
});
