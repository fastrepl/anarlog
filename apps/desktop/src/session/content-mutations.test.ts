import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  saveGeneratedSummary: vi.fn(
    async (
      _request: unknown,
    ): Promise<
      { status: "ok"; data: null } | { status: "error"; error: string }
    > => ({ status: "ok", data: null }),
  ),
}));

vi.mock("@anlg/plugin-template", () => ({
  commands: {
    saveGeneratedSummary: mocks.saveGeneratedSummary,
  },
}));

import { persistGeneratedEnhancedNote } from "./content-mutations";

describe("session content SQLite corrections", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("rejects when the Rust save fails", async () => {
    mocks.saveGeneratedSummary.mockResolvedValueOnce({
      status: "error",
      error: "database is locked",
    });
    await expect(
      persistGeneratedEnhancedNote({
        sessionId: "session-1",
        ownerUserId: "user-1",
        note: {
          id: "summary-1",
          currentContent: "old summary",
          currentContentFormat: "markdown",
          nextContent: '{"type":"doc"}',
        },
        tagNames: [],
      }),
    ).rejects.toThrow("database is locked");
  });
});
