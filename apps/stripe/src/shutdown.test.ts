import { expect, test } from "bun:test";

import { drainServer } from "./shutdown";

test("shutdown lets an accepted webhook finish and waits for the worker", async () => {
  const accepted = Promise.withResolvers<void>();
  const finishRequest = Promise.withResolvers<void>();
  const finishWorker = Promise.withResolvers<void>();
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    async fetch() {
      accepted.resolve();
      await finishRequest.promise;
      return Response.json({ received: true });
    },
  });
  try {
    const response = fetch(server.url, { method: "POST" }).then((value) =>
      value.json(),
    );
    await accepted.promise;
    let stopped = false;
    const shutdown = drainServer(server, () => finishWorker.promise).then(
      () => {
        stopped = true;
      },
    );
    finishRequest.resolve();
    expect(await response).toEqual({ received: true });
    expect(stopped).toBe(false);
    finishWorker.resolve();
    await shutdown;
    expect(stopped).toBe(true);
  } finally {
    finishRequest.resolve();
    finishWorker.resolve();
    await server.stop(true);
  }
});

test("a worker shutdown error does not cut off an accepted response", async () => {
  const accepted = Promise.withResolvers<void>();
  const finishRequest = Promise.withResolvers<void>();
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    async fetch() {
      accepted.resolve();
      await finishRequest.promise;
      return new Response("complete");
    },
  });
  try {
    const response = fetch(server.url).then((value) => value.text());
    await accepted.promise;
    const shutdown = drainServer(server, async () => {
      throw new Error("worker failed");
    });
    const result = shutdown.then(
      () => undefined,
      (error: unknown) => error,
    );
    finishRequest.resolve();
    expect(await response).toBe("complete");
    expect(await result).toBeInstanceOf(AggregateError);
  } finally {
    finishRequest.resolve();
    await server.stop(true);
  }
});

test("shutdown preserves a streaming response through its final chunk", async () => {
  const finishStream = Promise.withResolvers<void>();
  const encoder = new TextEncoder();
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch() {
      return new Response(
        new ReadableStream({
          async start(controller) {
            controller.enqueue(encoder.encode("first "));
            await finishStream.promise;
            controller.enqueue(encoder.encode("last"));
            controller.close();
          },
        }),
      );
    },
  });
  try {
    const response = await fetch(server.url);
    const reader = response.body!.getReader();
    expect(new TextDecoder().decode((await reader.read()).value)).toBe(
      "first ",
    );
    const shutdown = drainServer(server, async () => {});
    finishStream.resolve();
    expect(new TextDecoder().decode((await reader.read()).value)).toBe("last");
    expect((await reader.read()).done).toBe(true);
    await shutdown;
  } finally {
    finishStream.resolve();
    await server.stop(true);
  }
});
