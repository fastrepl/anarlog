import { describe, expect, it, vi } from "vitest";

import { startTrialOnce } from "./trial-start";

describe("startTrialOnce", () => {
  it("shares one successful trial start per user", async () => {
    const start = vi.fn().mockResolvedValue({ started: true });

    const [first, second] = await Promise.all([
      startTrialOnce("deduplicated-user", start),
      startTrialOnce("deduplicated-user", start),
    ]);

    expect(start).toHaveBeenCalledOnce();
    expect(first).toEqual({ started: true });
    expect(second).toEqual({ started: true });
  });

  it("allows a failed trial start to retry", async () => {
    const start = vi
      .fn()
      .mockRejectedValueOnce(new Error("Unavailable"))
      .mockResolvedValueOnce({ started: true });

    await expect(startTrialOnce("retry-user", start)).rejects.toThrow(
      "Unavailable",
    );
    await expect(startTrialOnce("retry-user", start)).resolves.toEqual({
      started: true,
    });
    expect(start).toHaveBeenCalledTimes(2);
  });
});
