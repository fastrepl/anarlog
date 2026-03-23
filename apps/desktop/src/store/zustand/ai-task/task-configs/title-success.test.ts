import type { LanguageModel } from "ai";
import { describe, expect, it, vi } from "vitest";

import type { TaskConfig } from ".";
import { titleSuccess } from "./title-success";

type TitleSuccessParams = Parameters<
  NonNullable<TaskConfig<"title">["onSuccess"]>
>[0];

function createParams(
  overrides: Partial<TitleSuccessParams> = {},
): TitleSuccessParams {
  const store = {
    setPartialRow: vi.fn(),
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
  it("persists trimmed title text", () => {
    const params = createParams({ text: "  Weekly sync  " });

    titleSuccess.onSuccess?.(params);

    expect(params.store.setPartialRow).toHaveBeenCalledWith(
      "sessions",
      "session-1",
      { title: "Weekly sync" },
    );
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
