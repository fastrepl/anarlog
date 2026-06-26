import { beforeEach, describe, expect, test, vi } from "vitest";

import { loadAllSessionData } from "./index";

const fsSyncMocks = vi.hoisted(() => ({
  scanAndRead: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));
vi.mock("@hypr/plugin-fs-sync", () => ({ commands: fsSyncMocks }));

describe("loadAllSessionData", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fsSyncMocks.scanAndRead.mockResolvedValue({
      status: "ok",
      data: { files: {}, dirs: [] },
    });
  });

  test("scans only metadata when content is excluded", async () => {
    await loadAllSessionData("/data", { includeContent: false });

    expect(fsSyncMocks.scanAndRead).toHaveBeenCalledWith(
      "/data/sessions",
      ["_meta.json"],
      true,
      null,
    );
  });

  test("scans metadata and content by default", async () => {
    await loadAllSessionData("/data");

    expect(fsSyncMocks.scanAndRead).toHaveBeenCalledWith(
      "/data/sessions",
      ["_meta.json", "transcript.json", "*.md"],
      true,
      null,
    );
  });
});
