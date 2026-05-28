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
