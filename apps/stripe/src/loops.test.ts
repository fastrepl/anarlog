import { describe, expect, it } from "bun:test";

import { sendLoopsTransactional } from "./loops";

const transactionalRequest = {
  apiKey: "loops-key",
  transactionalId: "transactional-123",
  email: "alex@example.com",
  dataVariables: { firstName: "Alex" },
  idempotencyKey: "stripe-event-123",
};

describe("sendLoopsTransactional", () => {
  it("sends idempotency in the header", async () => {
    let request: { url: string; init?: RequestInit } | undefined;

    await sendLoopsTransactional({
      ...transactionalRequest,
      fetcher: async (url, init) => {
        request = { url: String(url), init };
        return Response.json({ success: true });
      },
    });

    expect(request?.url).toBe("https://app.loops.so/api/v1/transactional");
    expect(new Headers(request?.init?.headers).get("Idempotency-Key")).toBe(
      "stripe-event-123",
    );
    expect(JSON.parse(String(request?.init?.body))).toEqual({
      transactionalId: "transactional-123",
      email: "alex@example.com",
      dataVariables: { firstName: "Alex" },
    });
  });

  it("accepts an already processed idempotency key", async () => {
    await expect(
      sendLoopsTransactional({
        ...transactionalRequest,
        fetcher: async () =>
          Response.json(
            {
              success: false,
              message: "Request has already been processed.",
            },
            { status: 409 },
          ),
      }),
    ).resolves.toBeUndefined();
  });

  it("retries Loops rate limits", async () => {
    let requests = 0;

    await sendLoopsTransactional({
      ...transactionalRequest,
      fetcher: async () => {
        requests += 1;
        if (requests === 1) {
          return Response.json(
            { success: false, message: "Too many requests." },
            { status: 429, headers: { "Retry-After": "0" } },
          );
        }
        return Response.json({ success: true });
      },
    });

    expect(requests).toBe(2);
  });
});
