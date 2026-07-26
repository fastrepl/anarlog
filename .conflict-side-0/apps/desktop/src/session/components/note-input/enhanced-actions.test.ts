import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  model: null as unknown,
  start: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: vi.fn() },
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("~/ai/hooks", () => ({
  useAITaskTask: () => ({
    isGenerating: false,
    isError: false,
    error: null,
    start: mocks.start,
    cancel: vi.fn(),
  }),
  useLanguageModel: () => mocks.model,
}));

vi.mock("~/ai/task-window-sync", () => ({
  isMainAITaskHostWindow: () => true,
  requestMainAITaskCancel: vi.fn(),
  requestMainEnhance: vi.fn(),
}));

vi.mock("~/session/queries", () => ({
  useEnhancedNote: () => ({ templateId: "template-1" }),
}));

import { useEnhancedNoteActions } from "./enhanced-actions";

describe("useEnhancedNoteActions", () => {
  beforeEach(() => {
    mocks.model = null;
    mocks.start.mockReset();
    mocks.toastError.mockReset();
  });

  it("shows a toast without entering an error state when Intelligence is not configured", async () => {
    const { result } = renderHook(() =>
      useEnhancedNoteActions({
        enhancedNoteId: "summary-1",
        sessionId: "session-1",
      }),
    );

    await act(() => result.current.onRegenerate(null));

    expect(mocks.toastError).toHaveBeenCalledWith(
      "Set up Intelligence in Settings before regenerating this summary.",
    );
    expect(mocks.start).not.toHaveBeenCalled();
    expect(result.current.isError).toBe(false);
    expect(result.current.error).toBeNull();
  });
});
