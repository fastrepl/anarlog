import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TaskConfig } from ".";
import { enhanceSuccess } from "./enhance-success";

import { createSummaryReadyNotificationKey } from "~/services/summary-ready-notification";

const mocks = vi.hoisted(() => ({
  isFocused: vi.fn().mockResolvedValue(true),
  showNotification: vi.fn().mockResolvedValue({ status: "ok", data: null }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isFocused: mocks.isFocused,
  }),
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: {
    showNotification: mocks.showNotification,
  },
}));

type EnhanceSuccessParams = Parameters<
  NonNullable<TaskConfig<"enhance">["onSuccess"]>
>[0];

function createParams(
  overrides: Partial<EnhanceSuccessParams> = {},
): EnhanceSuccessParams {
  const store = {
    setPartialRow: vi.fn(),
    getCell: vi.fn().mockReturnValue(""),
  } as unknown as EnhanceSuccessParams["store"];

  return {
    taskId: "note-1-enhance",
    text: "# Summary\n\n- Point",
    model: {} as LanguageModel,
    args: {
      sessionId: "session-1",
      enhancedNoteId: "note-1",
      templateId: undefined,
    },
    transformedArgs: {} as EnhanceSuccessParams["transformedArgs"],
    store,
    settingsStore: {} as EnhanceSuccessParams["settingsStore"],
    startTask: vi.fn().mockResolvedValue(undefined),
    getTaskState: vi.fn().mockReturnValue(undefined),
    ...overrides,
  };
}

describe("enhanceSuccess.onSuccess", () => {
  beforeEach(() => {
    mocks.isFocused.mockReset();
    mocks.isFocused.mockResolvedValue(true);
    mocks.showNotification.mockReset();
    mocks.showNotification.mockResolvedValue({ status: "ok", data: null });
  });

  it("persists enhanced note content as TipTap JSON string", async () => {
    const params = createParams();

    await enhanceSuccess.onSuccess?.(params);

    expect(params.store.setPartialRow).toHaveBeenCalledWith(
      "enhanced_notes",
      "note-1",
      expect.objectContaining({
        content: expect.any(String),
      }),
    );

    const persisted = (params.store.setPartialRow as ReturnType<typeof vi.fn>)
      .mock.calls[0][2].content;
    expect(() => JSON.parse(persisted)).not.toThrow();
  });

  it("starts title generation when session title is empty", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue(""),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    expect(startTask).toHaveBeenCalledWith("session-1-title", {
      model: params.model,
      taskType: "title",
      args: { sessionId: "session-1" },
    });
  });

  it("does not start title generation when title already exists", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue("Existing title"),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    expect(startTask).not.toHaveBeenCalled();
  });

  it("does not start title generation when title task is already running", async () => {
    const params = createParams({
      getTaskState: vi.fn().mockReturnValue({
        taskType: "title",
        status: "generating",
        streamedText: "",
        abortController: null,
        currentStep: undefined,
      }),
    });

    await enhanceSuccess.onSuccess?.(params);

    expect(params.startTask).not.toHaveBeenCalled();
  });

  it("shows a notification when summary generation finishes out of focus", async () => {
    mocks.isFocused.mockResolvedValue(false);
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn((table: string, _row: string, cell: string) => {
        if (table === "enhanced_notes" && cell === "title") {
          return "Summary";
        }
        if (table === "sessions" && cell === "title") {
          return "Weekly sync";
        }
        return "";
      }),
    } as unknown as EnhanceSuccessParams["store"];
    const params = createParams({ store });

    await enhanceSuccess.onSuccess?.(params);

    expect(mocks.showNotification).toHaveBeenCalledWith({
      key: createSummaryReadyNotificationKey("session-1", "note-1"),
      title: "Summary ready",
      message: "Weekly sync",
      timeout: null,
      source: null,
      start_time: null,
      participants: null,
      event_details: null,
      action_label: "Open summary",
      options: null,
    });
  });

  it("does not show a notification when the app is focused", async () => {
    const params = createParams();

    await enhanceSuccess.onSuccess?.(params);

    expect(mocks.showNotification).not.toHaveBeenCalled();
  });
});
