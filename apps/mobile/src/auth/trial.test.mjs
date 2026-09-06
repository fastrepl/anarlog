import assert from "node:assert/strict";
import test from "node:test";

import { enrollInTrial } from "./trial.ts";

function fixture(responses) {
  const requests = [];
  return {
    requests,
    options: {
      apiUrl: "https://api.anarlog.test/",
      accessToken: "synthetic-token",
      signal: new AbortController().signal,
      request: async (url, options) => {
        requests.push({ url, ...options });
        const response = responses.shift();
        if (response instanceof Error) throw response;
        return response;
      },
    },
  };
}

test("eligible mobile accounts start the shared server trial without supplying a new deadline", async () => {
  const { options, requests } = fixture([
    Response.json({ canStartTrial: true, reason: "eligible" }),
    Response.json({ started: true, reason: "started" }),
  ]);
  assert.equal(await enrollInTrial(options), true);
  assert.deepEqual(
    requests.map(({ url, method }) => [url, method ?? "GET"]),
    [
      ["https://api.anarlog.test/subscription/can-start-trial", "GET"],
      [
        "https://api.anarlog.test/subscription/start-trial?interval=monthly",
        "POST",
      ],
    ],
  );
  for (const request of requests) {
    assert.equal(request.headers.Authorization, "Bearer synthetic-token");
    assert.equal(request.signal, options.signal);
    assert.equal(request.body, undefined);
  }
});

test("previously used trials never send a start request", async () => {
  const { options, requests } = fixture([
    Response.json({ canStartTrial: false, reason: "not_eligible" }),
  ]);
  assert.equal(await enrollInTrial(options), false);
  assert.equal(requests.length, 1);
});

test("optional eligibility reasons are compatible with the subscription API", async () => {
  const { options } = fixture([
    Response.json({ canStartTrial: true }),
    Response.json({ started: true }),
  ]);
  assert.equal(await enrollInTrial(options), true);
});

test("a concurrent desktop activation is handled as an existing trial", async () => {
  const { options } = fixture([
    Response.json({ canStartTrial: true, reason: "eligible" }),
    Response.json({ started: false, reason: "not_eligible" }),
  ]);
  assert.equal(await enrollInTrial(options), false);
});

test("eligibility outages and malformed responses remain retryable", async () => {
  for (const response of [
    new Response(null, { status: 503 }),
    Response.json({ canStartTrial: false, reason: "error" }),
    Response.json({}),
  ]) {
    const { options, requests } = fixture([response]);
    await assert.rejects(enrollInTrial(options), /Could not check/);
    assert.equal(requests.length, 1);
  }
});

test("failed or ambiguous enrollment is not reported as a trial", async () => {
  for (const response of [
    new Response(null, { status: 500 }),
    Response.json({ started: false, reason: "error" }),
  ]) {
    const { options } = fixture([
      Response.json({ canStartTrial: true, reason: "eligible" }),
      response,
    ]);
    await assert.rejects(enrollInTrial(options), /Could not start/);
  }
});

test("account cancellation aborts enrollment", async () => {
  const controller = new AbortController();
  const { options } = fixture([]);
  options.signal = controller.signal;
  options.request = async (_url, { signal }) =>
    new Promise((_resolve, reject) => {
      signal.addEventListener("abort", () => reject(signal.reason), {
        once: true,
      });
    });
  const enrollment = enrollInTrial(options);
  controller.abort();
  await assert.rejects(enrollment, { name: "AbortError" });
});
