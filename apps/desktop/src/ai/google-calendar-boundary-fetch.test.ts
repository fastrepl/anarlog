import { describe, expect, it, vi } from "vitest";

import {
  createGoogleCalendarBoundaryFetch,
  GoogleCalendarRemoteAiBlockedError,
} from "./google-calendar-boundary-fetch";

describe("Google Calendar request-time AI boundary", () => {
  it("blocks before an off-device request is sent", async () => {
    const fetchImpl = vi.fn<typeof fetch>();
    const boundaryFetch = createGoogleCalendarBoundaryFetch(
      fetchImpl,
      async () => true,
    );

    await expect(boundaryFetch("https://model.example.com")).rejects.toThrow(
      GoogleCalendarRemoteAiBlockedError,
    );
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("sends the request only after Google data is conclusively absent", async () => {
    const response = new Response(null, { status: 204 });
    const fetchImpl = vi.fn<typeof fetch>(async () => response);
    const boundaryFetch = createGoogleCalendarBoundaryFetch(
      fetchImpl,
      async () => false,
    );

    await expect(boundaryFetch("https://model.example.com")).resolves.toBe(
      response,
    );
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  it("fails closed when the local data check fails", async () => {
    const fetchImpl = vi.fn<typeof fetch>();
    const boundaryFetch = createGoogleCalendarBoundaryFetch(
      fetchImpl,
      async () => {
        throw new Error("database unavailable");
      },
    );

    await expect(boundaryFetch("https://model.example.com")).rejects.toThrow(
      "database unavailable",
    );
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});
