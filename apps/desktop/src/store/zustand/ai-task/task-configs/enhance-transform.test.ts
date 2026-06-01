import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { enhanceTransform } from "./enhance-transform";

const mocks = vi.hoisted(() => ({
  collectEnhanceImageContext: vi.fn(),
  getTemplateById: vi.fn(),
}));

vi.mock("./enhance-images", () => ({
  collectEnhanceImageContext: mocks.collectEnhanceImageContext,
}));

vi.mock("~/templates/queries", () => ({
  getTemplateById: mocks.getTemplateById,
}));

vi.mock("~/stt/render-transcript", () => ({
  buildRenderTranscriptRequestFromStore: vi.fn(() => null),
  renderTranscriptSegments: vi.fn(),
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
    mocks.collectEnhanceImageContext.mockResolvedValue([]);
    mocks.getTemplateById.mockResolvedValue(null);
    consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleError.mockRestore();
  });

  it("uses the selected template when it can be loaded", async () => {
    mocks.getTemplateById.mockResolvedValue({
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
    mocks.getTemplateById.mockRejectedValue(new Error("Failed query"));

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

  it("collects image context from pre- and post-meeting memo content", async () => {
    const store = createStore();
    store.forEachRow.mockImplementation(
      (tableId: string, callback: (rowId: string) => void) => {
        if (tableId === "transcripts") {
          callback("transcript-1");
        }
      },
    );
    store.getCell.mockImplementation(
      (tableId: string, _rowId: string, cellId: string) => {
        if (tableId === "sessions" && cellId === "title") {
          return "Weekly Review";
        }
        if (tableId === "sessions" && cellId === "raw_md") {
          return "![post](asset://localhost/post.png)";
        }
        if (tableId === "transcripts" && cellId === "session_id") {
          return "session-1";
        }
        if (tableId === "transcripts" && cellId === "started_at") {
          return 100;
        }
        if (tableId === "transcripts" && cellId === "memo_md") {
          return "![pre](asset://localhost/pre.png)";
        }

        return "";
      },
    );

    await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
      store,
      {
        getValue: vi.fn((valueId: string) => {
          if (valueId === "current_llm_provider") {
            return "openai";
          }
          if (valueId === "current_llm_model") {
            return "gpt-4o";
          }
          if (valueId === "ai_language") {
            return "en";
          }

          return "";
        }),
      } as any,
    );

    expect(mocks.collectEnhanceImageContext).toHaveBeenCalledWith("session-1", [
      "![pre](asset://localhost/pre.png)",
      "![post](asset://localhost/post.png)",
    ]);
  });
});
