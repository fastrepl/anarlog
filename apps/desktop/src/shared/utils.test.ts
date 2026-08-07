import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getIdentifier: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: mocks.getIdentifier,
}));

import { getScheme } from "./utils";

describe("getScheme", () => {
  beforeEach(() => {
    mocks.getIdentifier.mockReset();
  });

  it.each([
    ["com.hyprnote.stable", "anarlog"],
    ["com.hyprnote.Hyprnote", "anarlog"],
    ["com.hyprnote.staging", "anarlog-staging"],
    ["com.hyprnote.dev", "anarlog-dev"],
    ["so.anarlog.Anarlog", "anarlog"],
    ["unknown", "anarlog"],
  ])("maps %s to %s", async (identifier, scheme) => {
    mocks.getIdentifier.mockResolvedValue(identifier);

    await expect(getScheme()).resolves.toBe(scheme);
  });
});
