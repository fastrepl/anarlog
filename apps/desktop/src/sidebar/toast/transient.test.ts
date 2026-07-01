import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { showTransientToast, useTransientToast } from "./transient";

describe("transient toast store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useTransientToast.getState().clearToast();
  });

  afterEach(() => {
    useTransientToast.getState().clearToast();
    vi.useRealTimers();
  });

  it("keeps a toast visible when duration is disabled", () => {
    showTransientToast(
      {
        id: "persistent-toast",
        description: "Persistent warning",
      },
      { durationMs: null },
    );

    act(() => {
      vi.advanceTimersByTime(10_000);
    });

    expect(useTransientToast.getState().toast?.id).toBe("persistent-toast");
  });
});
