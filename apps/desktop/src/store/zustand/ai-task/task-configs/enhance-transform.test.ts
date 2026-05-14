import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { enhanceTransform } from "./enhance-transform";

const getTemplateByIdMock = vi.hoisted(() => vi.fn());

vi.mock("~/templates/queries", () => ({
  getTemplateById: getTemplateByIdMock,
}));

function createStore() {
  return {
    forEachRow: vi.fn(),
    getCell: vi.fn((tableId: string, _rowId: string, cellId: string) => {
      if (tableId === "sessions" && cellId === "title") {
        return "Weekly Review";
      }

      return "";
    }),
    getRow: vi.fn((tableId: string) => {
      if (tableId === "sessions") {
        return { title: "Weekly Review" };
      }

      return undefined;
    }),
  } as any;
}

function createSettingsStore() {
  return {
    getValue: vi.fn(() => "en"),
  } as any;
}

describe("enhanceTransform.transformArgs", () => {
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    getTemplateByIdMock.mockResolvedValue(null);
    consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleError.mockRestore();
  });

  it("uses the selected template when it can be loaded", async () => {
    getTemplateByIdMock.mockResolvedValue({
      title: "Standup",
      description: "Daily sync",
      sections: [{ title: "Updates", description: null }],
    });

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      createStore(),
      createSettingsStore(),
    );

    expect(result.template).toEqual({
      title: "Standup",
      description: "Daily sync",
      sections: [{ title: "Updates", description: null }],
    });
  });

  it("falls back to generic enhancement when template loading fails", async () => {
    getTemplateByIdMock.mockRejectedValue(new Error("Failed query"));

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      createStore(),
      createSettingsStore(),
    );

    expect(result.template).toBeNull();
    expect(result.session.title).toBe("Weekly Review");
    expect(consoleError).toHaveBeenCalledWith(
      "[enhance] failed to load template",
      expect.any(Error),
    );
  });
});
