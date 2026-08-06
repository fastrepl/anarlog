import { beforeEach, describe, expect, it, vi } from "vitest";

const { setCompetingApplicationTerminationPaused } = vi.hoisted(() => ({
  setCompetingApplicationTerminationPaused: vi.fn(),
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: { setCompetingApplicationTerminationPaused },
}));

import { pauseCompetingApplicationTermination } from "./termination-pause";

describe("competitor termination pause", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setCompetingApplicationTerminationPaused.mockResolvedValue({
      status: "ok",
      data: null,
    });
  });

  it("keeps competitors open for the import workflow", async () => {
    const resume = pauseCompetingApplicationTermination();

    await Promise.resolve();
    resume();
    await Promise.resolve();

    expect(setCompetingApplicationTerminationPaused.mock.calls).toEqual([
      [true],
      [false],
    ]);
  });
});
