import { hasGoogleCalendarData } from "./google-calendar-data";

export class GoogleCalendarRemoteAiBlockedError extends Error {
  constructor() {
    super(
      "Remote AI is disabled while Google Calendar data remains on this device.",
    );
    this.name = "GoogleCalendarRemoteAiBlockedError";
  }
}

export function createGoogleCalendarBoundaryFetch(
  fetchImpl: typeof fetch,
  checkHasGoogleCalendarData: () => Promise<boolean> = hasGoogleCalendarData,
): typeof fetch {
  return async (input, init) => {
    if (await checkHasGoogleCalendarData()) {
      throw new GoogleCalendarRemoteAiBlockedError();
    }
    return fetchImpl(input, init);
  };
}
