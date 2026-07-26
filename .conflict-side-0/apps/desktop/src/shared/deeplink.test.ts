import { describe, expect, it, vi } from "vitest";

import { subscribeThenDrainDeepLinks } from "./deeplink";

describe("desktop deep links", () => {
  it("subscribes before draining cold-start callbacks", async () => {
    const order: string[] = [];
    const handle = vi.fn(() => {
      order.push("handle");
    });
    const deepLink = { to: "/billing/refresh", search: {} } as const;

    const unlisten = await subscribeThenDrainDeepLinks({
      listen: async () => {
        order.push("listen");
        return () => {};
      },
      takePendingDeepLinks: async () => {
        order.push("take");
        return { status: "ok", data: [deepLink] };
      },
      handle,
    });

    expect(order).toEqual(["listen", "take", "handle"]);
    expect(handle).toHaveBeenCalledWith(deepLink);
    expect(unlisten).toEqual(expect.any(Function));
  });
});
