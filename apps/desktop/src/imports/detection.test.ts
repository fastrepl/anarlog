import { beforeEach, describe, expect, it, vi } from "vitest";

const { listInstalledApplications } = vi.hoisted(() => ({
  listInstalledApplications: vi.fn(),
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: { listInstalledApplications },
}));

import { detectImportSources } from "./detection";

describe("import source detection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listInstalledApplications.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.granola.app", name: "Granola" }],
    });
  });

  it("detects installed import sources without terminating them", async () => {
    const result = await detectImportSources();

    expect(listInstalledApplications).toHaveBeenCalledOnce();
    expect(result.map((provider) => provider.id)).toEqual(["granola"]);
  });
});
