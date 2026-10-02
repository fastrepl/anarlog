import { expect, it } from "vitest";

import { resolveDeviceKind } from "./device-kind";

it("preserves desktop classification for ambiguous phone-related device names", () => {
  for (const [kind, name, expected] of [
    [undefined, "iPhone development Mac", "desktop"],
    [undefined, "John's iPhone", "mobile"],
    [undefined, "SM-S918B", "mobile"],
    ["desktop", "iPhone", "desktop"],
    ["watch", "John's device", "watch"],
  ] as const) {
    expect(resolveDeviceKind(kind, name)).toBe(expected);
  }
});
