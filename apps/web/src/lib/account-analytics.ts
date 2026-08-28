export type AccountAnalyticsEvent = {
  id: string;
  event_name: "account_created" | "account_confirmed";
  user_id: string;
  occurred_at: string;
  properties: Record<string, unknown>;
  historical: boolean;
};

type Fetcher = (...args: Parameters<typeof fetch>) => ReturnType<typeof fetch>;

const ACCOUNT_GROUP_TYPE = "account";

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
  fetcher?: Fetcher;
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
      batch: events.flatMap((event) => {
        const capturedEvent = {
          event: event.event_name,
          properties: {
            ...event.properties,
            distinct_id: event.user_id,
            $groups: {
              [ACCOUNT_GROUP_TYPE]: event.user_id,
            },
            $insert_id: `${event.event_name}:${event.user_id}`,
          },
          timestamp: event.occurred_at,
        };

        if (event.event_name !== "account_created") {
          return [capturedEvent];
        }

        const setProperties =
          event.properties["$set"] &&
          typeof event.properties["$set"] === "object" &&
          !Array.isArray(event.properties["$set"])
            ? (event.properties["$set"] as Record<string, unknown>)
            : {};
        const email =
          typeof setProperties["email"] === "string"
            ? setProperties["email"]
            : null;

        return [
          {
            event: "$groupidentify",
            properties: {
              distinct_id: event.user_id,
              $group_type: ACCOUNT_GROUP_TYPE,
              $group_key: event.user_id,
              $group_set: {
                name: email ?? event.user_id,
                email,
                created_at:
                  setProperties["account_created_at"] ?? event.occurred_at,
              },
              $insert_id: `account-group:${event.user_id}`,
            },
            timestamp: event.occurred_at,
          },
          capturedEvent,
        ];
      }),
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
