import { describe, expect, it } from "bun:test";

import { sanitizeErrorEvent } from "./error-reporting";

describe("sanitizeErrorEvent", () => {
  it("removes customer error content while preserving exception diagnostics", () => {
    const event = sanitizeErrorEvent({
      type: undefined,
      message: "customer@example.com",
      logentry: { message: "payment token" },
      exception: {
        values: [
          {
            type: "StripeError",
            value: "private request details",
            mechanism: {
              type: "generic",
              handled: false,
              data: { customer: "cus_private" },
            },
          },
        ],
      },
      extra: { customer: "cus_private" },
      breadcrumbs: [
        {
          category: "console",
          message: "private card details",
          data: { arguments: ["private card details"] },
        },
        {
          category: "navigation",
          data: {
            from: "https://anarlog.so/?token=secret",
            to: "https://anarlog.so/billing?customer=private",
          },
        },
      ],
    });

    expect(event.message).toBeUndefined();
    expect(event.logentry).toBeUndefined();
    expect(event.extra).toBeUndefined();
    expect(event.exception?.values).toEqual([
      {
        type: "StripeError",
        value: "StripeError captured",
        mechanism: { type: "generic", handled: false },
      },
    ]);
    expect(event.breadcrumbs).toEqual([
      {
        category: "console",
        level: undefined,
        timestamp: undefined,
        type: undefined,
      },
      {
        category: "navigation",
        level: undefined,
        timestamp: undefined,
        type: undefined,
        data: {
          from: "https://anarlog.so/",
          to: "https://anarlog.so/billing",
        },
      },
    ]);
  });
});
