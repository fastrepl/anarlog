import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  model: null as unknown,
  start: vi.fn(),
  toastError: vi.fn(),
  toastWarning: vi.fn(),
  isMainWindow: true,
  requestMainEnhance: vi.fn(),
  snapshot: null as unknown,
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: { event: vi.fn() },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError, warning: mocks.toastWarning },
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
  isMainAITaskHostWindow: () => mocks.isMainWindow,
  requestMainAITaskCancel: vi.fn(),
  requestMainEnhance: mocks.requestMainEnhance,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: () => Promise.resolve(mocks.snapshot),
}));

vi.mock("~/session/queries", () => ({
  useEnhancedNote: () => ({ templateId: "template-1" }),
}));

import { useEnhancedNoteActions } from "./enhanced-actions";

function renderActions() {
  return renderHook(() =>
    useEnhancedNoteActions({
      enhancedNoteId: "summary-1",
      sessionId: "session-1",
    }),
  );
}

describe("useEnhancedNoteActions", () => {
  beforeEach(() => {
    mocks.model = null;
    mocks.isMainWindow = true;
    mocks.snapshot = null;
    mocks.start.mockReset();
    mocks.toastError.mockReset();
    mocks.toastWarning.mockReset();
    mocks.requestMainEnhance.mockReset();
  });

  it("shows a toast without entering an error state when Intelligence is not configured", async () => {
    const { result } = renderActions();

    await act(() => result.current.onRegenerate(null));

    expect(mocks.toastError).toHaveBeenCalledWith(
      "Set up Intelligence in Settings before regenerating this summary.",
    );
    expect(mocks.start).not.toHaveBeenCalled();
    expect(result.current.isError).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("shows the too-short toast instead of forwarding to main from a standalone window", async () => {
    mocks.model = { id: "model-1" };
    mocks.isMainWindow = false;
    mocks.snapshot = {
      transcripts: [{ words: [{ text: "hi" }, { text: "there" }] }],
    };

    const { result } = renderActions();

    await act(() => result.current.onRegenerate(null));

    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "Summary wasn't generated",
      expect.objectContaining({ id: "auto-summary-too-short-session-1" }),
    );
    expect(mocks.requestMainEnhance).not.toHaveBeenCalled();
    expect(mocks.start).not.toHaveBeenCalled();
  });

  it("forwards eligible sessions to the main window", async () => {
    mocks.model = { id: "model-1" };
    mocks.isMainWindow = false;
    mocks.snapshot = {
      transcripts: [
        {
          words: Array.from({ length: 40 }, (_, index) => ({
            text: `word-${index}`,
          })),
        },
      ],
    };

    const { result } = renderActions();

    await act(() => result.current.onRegenerate(null));

    expect(mocks.toastWarning).not.toHaveBeenCalled();
    expect(mocks.requestMainEnhance).toHaveBeenCalledWith("session-1", {
      templateId: "template-1",
      targetNoteId: "summary-1",
    });
    expect(mocks.start).not.toHaveBeenCalled();
  });
});
