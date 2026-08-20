import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  base: vi.fn(),
  success: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
  message: vi.fn(),
  loading: vi.fn(),
  custom: vi.fn(),
  promise: vi.fn(),
  dismiss: vi.fn(),
  getHistory: vi.fn(),
  getToasts: vi.fn(),
}));

vi.mock("sonner", () => ({
  Toaster: () => null,
  toast: Object.assign(mocks.base, {
    success: mocks.success,
    info: mocks.info,
    warning: mocks.warning,
    error: mocks.error,
    message: mocks.message,
    loading: mocks.loading,
    custom: mocks.custom,
    promise: mocks.promise,
    dismiss: mocks.dismiss,
    getHistory: mocks.getHistory,
    getToasts: mocks.getToasts,
  }),
}));

import { sonnerToast, TOAST_DURATIONS } from "@anlg/ui/components/ui/toast";

describe("toast durations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("applies the default duration for each timed toast category", () => {
    expect(TOAST_DURATIONS.error).toBe(5_000);

    sonnerToast.success("Saved");
    sonnerToast.info("Heads up");
    sonnerToast.warning("Check this");
    sonnerToast.error("Try again");
    sonnerToast.message("Updated");

    expect(mocks.success).toHaveBeenCalledWith("Saved", {
      duration: TOAST_DURATIONS.success,
    });
    expect(mocks.info).toHaveBeenCalledWith("Heads up", {
      duration: TOAST_DURATIONS.info,
    });
    expect(mocks.warning).toHaveBeenCalledWith("Check this", {
      duration: TOAST_DURATIONS.warning,
    });
    expect(mocks.error).toHaveBeenCalledWith("Try again", {
      duration: TOAST_DURATIONS.error,
    });
    expect(mocks.message).toHaveBeenCalledWith("Updated", {
      duration: TOAST_DURATIONS.info,
    });
  });

  it("preserves explicit durations and leaves loading toasts condition-bound", () => {
    sonnerToast.warning("Keep this", { duration: Infinity, id: "warning" });
    sonnerToast.loading("Working", { id: "loading" });

    expect(mocks.warning).toHaveBeenCalledWith("Keep this", {
      duration: Infinity,
      id: "warning",
    });
    expect(mocks.loading).toHaveBeenCalledWith("Working", { id: "loading" });
  });

  it("caps error toasts at five seconds even when callers request longer", () => {
    sonnerToast.error("Try again", { duration: Infinity, id: "error" });
    sonnerToast.error("Custom window", { duration: 12_000, id: "custom" });
    sonnerToast.error("Shorter window", { duration: 2_000, id: "short" });

    expect(mocks.error).toHaveBeenCalledWith("Try again", {
      duration: TOAST_DURATIONS.error,
      id: "error",
    });
    expect(mocks.error).toHaveBeenCalledWith("Custom window", {
      duration: TOAST_DURATIONS.error,
      id: "custom",
    });
    expect(mocks.error).toHaveBeenCalledWith("Shorter window", {
      duration: 2_000,
      id: "short",
    });
  });
});
