import { describe, expect, it, vi } from "vitest";

import { handleMainEnhanceRequest } from "./task-window-sync";

describe("handleMainEnhanceRequest", () => {
  it.each(["if_empty", "regenerate"] as const)(
    "routes %s through the retry-aware auto-enhance entrypoint",
    async (mode) => {
      const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);
      const enhance = vi.fn().mockResolvedValue({ type: "no_model" });

      await handleMainEnhanceRequest(
        {
          requestAutoEnhance,
          enhance,
        },
        {
          sessionId: "session-1",
          auto: mode,
        },
      );

      expect(requestAutoEnhance).toHaveBeenCalledWith("session-1", mode);
      expect(enhance).not.toHaveBeenCalled();
    },
  );

  it("keeps explicit enhancement requests on the direct path", async () => {
    const requestAutoEnhance = vi.fn().mockResolvedValue(undefined);
    const enhance = vi.fn().mockResolvedValue({ type: "no_model" });
    const opts = { isAuto: false, templateId: "template-1" };

    await handleMainEnhanceRequest(
      {
        requestAutoEnhance,
        enhance,
      },
      {
        sessionId: "session-1",
        opts,
      },
    );

    expect(enhance).toHaveBeenCalledWith("session-1", opts);
    expect(requestAutoEnhance).not.toHaveBeenCalled();
  });
});
