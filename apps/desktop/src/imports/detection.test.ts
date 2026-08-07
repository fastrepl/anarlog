import { beforeEach, describe, expect, it, vi } from "vitest";

const { getInstalledApplicationIcons, listInstalledApplications } = vi.hoisted(
  () => ({
    getInstalledApplicationIcons: vi.fn(),
    listInstalledApplications: vi.fn(),
  }),
);

vi.mock("@anlg/plugin-detect", () => ({
  commands: { getInstalledApplicationIcons, listInstalledApplications },
}));

import { detectImportSources } from "./detection";

describe("import source detection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getInstalledApplicationIcons.mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "com.granola.app",
          dataUrl: "data:image/png;base64,granola",
        },
      ],
    });
    listInstalledApplications.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.granola.app", name: "Granola" }],
    });
  });

  it("detects installed import sources without terminating them", async () => {
    const result = await detectImportSources();

    expect(listInstalledApplications).toHaveBeenCalledOnce();
    expect(getInstalledApplicationIcons).toHaveBeenCalledWith([
      "com.granola.app",
    ]);
    expect(result.map((provider) => provider.id)).toEqual(["granola"]);
    expect(result[0]?.iconUrl).toBe("data:image/png;base64,granola");
  });
});
