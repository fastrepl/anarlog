import { expect, test } from "bun:test";

import { drainBillingRuntime } from "./billing-runtime";

test("API drain completes accepted forwarding before stopping webhooks", async () => {
  const accepted = Promise.withResolvers<void>();
  const complete = Promise.withResolvers<void>();
  const exited = Promise.withResolvers<number>();
  const signals: string[] = [];
  const worker = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    async fetch() {
      accepted.resolve();
      await complete.promise;
      return new Response("processed once");
    },
  });
  let stopped = false;
  const response = fetch(worker.url, {
    method: "POST",
    body: "signed payload",
  }).then((r) => r.text());
  await accepted.promise;
  const shutdown = drainBillingRuntime(
    {
      kill(signal) {
        signals.push(String(signal));
      },
      exited: exited.promise,
      exitCode: null,
    },
    async () => {
      stopped = true;
      await worker.stop();
    },
  );
  expect(signals).toEqual(["SIGUSR1"]);
  expect(stopped).toBe(false);
  complete.resolve();
  expect(await response).toBe("processed once");
  expect(stopped).toBe(false);
  exited.resolve(0);
  await shutdown;
  expect(stopped).toBe(true);
});

test("failed API still drains local work and reports failure", async () => {
  let stopped = false;
  await expect(
    drainBillingRuntime(
      {
        kill() {
          throw new Error("must not signal an exited child");
        },
        exited: Promise.resolve(1),
        exitCode: 1,
      },
      async () => {
        stopped = true;
      },
    ),
  ).rejects.toThrow("Billing API exited with code 1");
  expect(stopped).toBe(true);
});
