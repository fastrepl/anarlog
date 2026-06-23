import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TaskConfig } from ".";
import { titleSuccess } from "./title-success";

import { useLiveTitle } from "~/store/zustand/live-title";

type TitleSuccessParams = Parameters<
  NonNullable<TaskConfig<"title">["onSuccess"]>
>[0];

function createParams(
  overrides: Partial<TitleSuccessParams> = {},
): TitleSuccessParams {
  const store = {
    setPartialRow: vi.fn(),
    getCell: vi.fn().mockReturnValue(""),
    forEachRow: vi.fn(),
  } as unknown as TitleSuccessParams["store"];

  return {
    taskId: "session-1-title",
    text: "Meeting title",
    model: {} as LanguageModel,
    args: { sessionId: "session-1" },
    transformedArgs: {} as TitleSuccessParams["transformedArgs"],
    store,
    settingsStore: {} as TitleSuccessParams["settingsStore"],
    startTask: vi.fn().mockResolvedValue(undefined),
    getTaskState: vi.fn().mockReturnValue(undefined),
    ...overrides,
  };
}

describe("titleSuccess.onSuccess", () => {
  beforeEach(() => {
    useLiveTitle.setState({ titles: {} });
  });

  it("persists trimmed title text", () => {
    const params = createParams({ text: "  Weekly sync  " });

    titleSuccess.onSuccess?.(params);

    expect(params.store.setPartialRow).toHaveBeenCalledWith(
      "sessions",
      "session-1",
      { title: "Weekly sync" },
    );
  });

  it("backfills generated titles into existing enhanced note content", () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn((table, row, cell) => {
        if (table === "sessions" && cell === "title") return "";
        if (table === "sessions" && cell === "raw_md") return "";
        if (table === "enhanced_notes" && cell === "session_id") {
          return row === "note-1" ? "session-1" : "other-session";
        }
        if (table === "enhanced_notes" && cell === "content") {
          return JSON.stringify({
            type: "doc",
            content: [
              {
                type: "heading",
                attrs: { level: 1 },
                content: [{ type: "text", text: "Summary Section" }],
              },
            ],
          });
        }
        return "";
      }),
      forEachRow: vi.fn((_table, callback) => {
        callback("note-1");
        callback("note-2");
      }),
    } as unknown as TitleSuccessParams["store"];
    const params = createParams({ store, text: "  Weekly sync  " });

    titleSuccess.onSuccess?.(params);

    const enhancedUpdate = (
      store.setPartialRow as ReturnType<typeof vi.fn>
    ).mock.calls.find(([table]) => table === "enhanced_notes");
    expect(enhancedUpdate?.[0]).toBe("enhanced_notes");
    expect(enhancedUpdate?.[1]).toBe("note-1");
    expect(JSON.parse(enhancedUpdate?.[2].content)).toMatchObject({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Summary Section" }],
        },
      ],
    });
  });

  it("does not overwrite an existing session title", () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue("Custom title"),
    } as unknown as TitleSuccessParams["store"];
    const params = createParams({ store });

    titleSuccess.onSuccess?.(params);

    expect(store.setPartialRow).not.toHaveBeenCalled();
  });

  it("does not overwrite an active title edit", () => {
    useLiveTitle.getState().setTitle("session-1", "Custom title");
    const params = createParams();

    titleSuccess.onSuccess?.(params);

    expect(params.store.setPartialRow).not.toHaveBeenCalled();
  });

  it("does not write a generated title while an active edit is blank", () => {
    useLiveTitle.getState().setTitle("session-1", "");
    const params = createParams();

    titleSuccess.onSuccess?.(params);

    expect(params.store.setPartialRow).not.toHaveBeenCalled();
  });

  it("ignores empty or placeholder title outputs", () => {
    const emptyParams = createParams({ text: "   " });
    titleSuccess.onSuccess?.(emptyParams);
    expect(emptyParams.store.setPartialRow).not.toHaveBeenCalled();

    const placeholderParams = createParams({ text: "<EMPTY>" });
    titleSuccess.onSuccess?.(placeholderParams);
    expect(placeholderParams.store.setPartialRow).not.toHaveBeenCalled();
  });
});
