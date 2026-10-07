import { createHash } from "node:crypto";

export async function sendReferralOutcome({
  apiKey,
  event,
  referralId,
  referrerUserId,
  referredUserId,
  timestamp,
  fetcher = fetch,
}: {
  apiKey?: string;
  event: "referral_trial_started" | "referral_reward_applied";
  referralId: string;
  referrerUserId: string;
  referredUserId: string;
  timestamp: Date;
  fetcher?: (url: string, init: RequestInit) => Promise<Response>;
}) {
  if (!apiKey) return false;
  const insertId = `${event}:${referralId}`;
  const bytes = createHash("sha256").update(insertId).digest().subarray(0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x80;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString("hex");
  const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  // A confirmed HTTP response lets the durable audit row track delivery. SDK
  // queue acceptance alone cannot prove an event reached PostHog.
  const response = await fetcher("https://us.i.posthog.com/capture/", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    signal: AbortSignal.timeout(2000),
    body: JSON.stringify({
      api_key: apiKey,
      event,
      uuid,
      timestamp: timestamp.toISOString(),
      properties: {
        distinct_id:
          event === "referral_reward_applied" ? referrerUserId : referredUserId,
        $insert_id: insertId,
        referral_id: referralId,
        referrer_user_id: referrerUserId,
        referred_user_id: referredUserId,
        reward_policy: "trial_month",
        surface: "stripe",
        analytics_schema_version: 1,
      },
    }),
  });
  if (!response.ok)
    throw new Error(`PostHog capture failed with ${response.status}`);
  return true;
}
