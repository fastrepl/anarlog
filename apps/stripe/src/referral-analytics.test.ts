import { expect, test } from "bun:test";

import { sendReferralOutcome } from "./referral-analytics";

test("trial and reward retries keep their IDs and timestamps and belong to the affected account", async () => {
  const payloads: {
    uuid: string;
    timestamp: string;
    properties: { distinct_id: string; $insert_id: string };
  }[] = [];
  for (const event of [
    "referral_trial_started",
    "referral_reward_applied",
  ] as const) {
    const send = () =>
      sendReferralOutcome({
        apiKey: "test",
        event,
        referralId: "invite-1",
        referrerUserId: "alice",
        referredUserId: "bob",
        timestamp: new Date("2026-10-07T00:00:00Z"),
        fetcher: async (_url, init) => {
          payloads.push(JSON.parse(String(init?.body)));
          return new Response(null, {
            status: payloads.length % 2 ? 503 : 200,
          });
        },
      });
    await expect(send()).rejects.toThrow("503");
    expect(await send()).toBe(true);
  }
  expect(payloads[0]).toEqual(payloads[1]);
  expect(payloads[2]).toEqual(payloads[3]);
  expect(payloads[0].properties.distinct_id).toBe("bob");
  expect(payloads[2].properties.distinct_id).toBe("alice");
  expect(payloads[0].uuid).not.toBe(payloads[2].uuid);
});
