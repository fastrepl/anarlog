import { describe, expect, it, vi } from "vitest";

import { waitForBillingUpdate } from "./billing";

describe("waitForBillingUpdate", () => {
  it("refreshes claims after Stripe has had time to propagate", async () => {
    vi.useFakeTimers();
    const refreshSession = vi.fn(async () => undefined);

    const waiting = waitForBillingUpdate(refreshSession, 3_000);

    expect(refreshSession).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(3_000);
    await waiting;

    expect(refreshSession).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});
