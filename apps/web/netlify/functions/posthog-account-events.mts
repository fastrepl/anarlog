import { createClient } from "@supabase/supabase-js";

import {
  groupAccountAnalyticsEvents,
  sendPostHogBatch,
  type AccountAnalyticsEvent,
} from "../../src/lib/account-analytics.ts";

function requireEnvironmentVariable(name: string) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

export default async () => {
  if (process.env.CONTEXT !== "production") {
    throw new Error("Account analytics delivery is production-only");
  }

  const supabase = createClient(
    requireEnvironmentVariable("SUPABASE_URL"),
    requireEnvironmentVariable("SUPABASE_SERVICE_ROLE_KEY"),
    {
      auth: {
        autoRefreshToken: false,
        persistSession: false,
      },
    },
  );
  const projectToken = requireEnvironmentVariable("VITE_POSTHOG_API_KEY");
  const host = process.env.VITE_POSTHOG_HOST ?? "https://us.i.posthog.com";

  const { data: reconciliation, error: reconciliationError } =
    await supabase.rpc("reconcile_account_analytics_events");
  if (reconciliationError) {
    throw reconciliationError;
  }

  const leaseId = crypto.randomUUID();
  const { data, error: claimError } = await supabase.rpc(
    "claim_account_analytics_events",
    {
      p_lease_id: leaseId,
      p_limit: 500,
      p_lease_seconds: 300,
    },
  );
  if (claimError) {
    throw claimError;
  }

  const events = (data ?? []) as AccountAnalyticsEvent[];
  const failures: Error[] = [];
  let delivered = 0;

  for (const group of groupAccountAnalyticsEvents(events)) {
    const eventIds = group.map((event) => event.id);

    try {
      await sendPostHogBatch({
        events: group,
        projectToken,
        host,
      });

      const { data: completed, error: completionError } = await supabase.rpc(
        "complete_account_analytics_events",
        {
          p_lease_id: leaseId,
          p_event_ids: eventIds,
        },
      );
      if (completionError) {
        throw completionError;
      }
      if (completed !== eventIds.length) {
        throw new Error(
          `Completed ${completed} of ${eventIds.length} account analytics events`,
        );
      }
      delivered += eventIds.length;
    } catch (error) {
      const failure = error instanceof Error ? error : new Error(String(error));
      failures.push(failure);

      const { error: failureError } = await supabase.rpc(
        "fail_account_analytics_events",
        {
          p_lease_id: leaseId,
          p_event_ids: eventIds,
          p_error: failure.message,
        },
      );
      if (failureError) {
        failures.push(failureError);
      }
    }
  }

  console.log(
    JSON.stringify({
      reconciliation: reconciliation?.[0] ?? null,
      claimed: events.length,
      delivered,
      failed: events.length - delivered,
    }),
  );

  if (failures.length > 0) {
    throw new AggregateError(failures, "Account analytics delivery failed");
  }
};

export const config = {
  schedule: "*/5 * * * *",
};
