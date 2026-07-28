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
      console.warn(`[analytics] failed to record ${event}`, error);
    });
  } catch (error) {
    console.warn(`[analytics] failed to record ${event}`, error);
  }
}
