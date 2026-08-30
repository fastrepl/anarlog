import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: () => ({ data: [] }),
}));

import { loadSessionSummariesByFolder } from "./folders";

describe("folder session queries", () => {
  beforeEach(() => {
    mocks.execute.mockReset();
  });

  it("loads unfiled sessions", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "unfiled",
        title: "Scratch",
        created_at: "2026-08-01T00:00:00.000Z",
        event_json: "",
        folder_path: "",
      },
    ]);

    await expect(loadSessionSummariesByFolder("")).resolves.toEqual([
      {
        id: "unfiled",
        title: "Scratch",
        created_at: "2026-08-01T00:00:00.000Z",
        event_json: "",
      },
    ]);
    expect(mocks.execute).toHaveBeenCalledWith(
      expect.stringContaining("folder_path = ''"),
      [],
    );
  });

  it("keeps nested paths under the top-level folder name", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "nested",
        title: "Week 3",
        created_at: "2026-08-10T00:00:00.000Z",
        event_json: "",
        folder_path: "CS 101/week-3",
      },
      {
        id: "other",
        title: "Other class",
        created_at: "2026-08-11T00:00:00.000Z",
        event_json: "",
        folder_path: "CS 101 Extra",
      },
    ]);

    await expect(loadSessionSummariesByFolder("CS 101")).resolves.toEqual([
      {
        id: "nested",
        title: "Week 3",
        created_at: "2026-08-10T00:00:00.000Z",
        event_json: "",
      },
    ]);
    expect(mocks.execute).toHaveBeenCalledWith(
      expect.stringContaining("folder_path = ?"),
      ["CS 101", "CS 101/%", "CS 101\\%"],
    );
  });
});
