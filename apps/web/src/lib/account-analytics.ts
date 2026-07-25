export type AccountAnalyticsEvent = {
  id: string;
  event_name: "account_created" | "account_confirmed";
  user_id: string;
  occurred_at: string;
  properties: Record<string, unknown>;
  historical: boolean;
};

export function groupAccountAnalyticsEvents(events: AccountAnalyticsEvent[]) {
  return [
    events.filter((event) => !event.historical),
    events.filter((event) => event.historical),
  ].filter((group) => group.length > 0);
}

export async function sendPostHogBatch({
  events,
  projectToken,
  host,
  fetcher = fetch,
}: {
  events: AccountAnalyticsEvent[];
  projectToken: string;
  host: string;
  fetcher?: typeof fetch;
}) {
  if (events.length === 0) {
    return;
  }

  const response = await fetcher(`${host.replace(/\/+$/, "")}/batch/`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      api_key: projectToken,
      historical_migration: events.every((event) => event.historical),
      batch: events.map((event) => ({
        event: event.event_name,
        properties: {
          ...event.properties,
          distinct_id: event.user_id,
          $insert_id: `${event.event_name}:${event.user_id}`,
        },
        timestamp: event.occurred_at,
      })),
    }),
  });

  if (!response.ok) {
    throw new Error(
      `PostHog batch rejected with ${response.status}: ${await response.text()}`,
    );
  }

  const result = (await response.json()) as { status?: unknown };
  if (result.status !== "Ok") {
    throw new Error("PostHog batch returned an invalid acknowledgement");
  }
}
