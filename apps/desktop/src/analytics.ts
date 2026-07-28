import {
  commands as analyticsCommands,
  type JsonValue,
} from "@hypr/plugin-analytics";

export function trackAnalyticsEvent(
  event: string,
  properties: Record<string, JsonValue> = {},
) {
  try {
    const capture = analyticsCommands.eventFireAndForget;
    if (typeof capture !== "function") return;

    const pending = capture({
      event,
      ...properties,
    });
    void pending.catch((error: unknown) => {
      console.error(`[analytics] failed to record ${event}`, error);
    });
  } catch (error) {
    console.error(`[analytics] failed to record ${event}`, error);
  }
}
