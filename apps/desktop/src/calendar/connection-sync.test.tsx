import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { getCalendarConnectionKey } from "./components/shared";
import { useSyncWhenCalendarConnectionsChange } from "./hooks";

describe("getCalendarConnectionKey", () => {
  it("ignores non-calendar integrations and orders connection ids", () => {
    expect(
      getCalendarConnectionKey([
        { connection_id: "outlook-2", integration_id: "outlook" },
        { connection_id: "slack-1", integration_id: "slack" },
        { connection_id: "google-1", integration_id: "google-calendar" },
      ]),
    ).toBe("google-1,outlook-2");
  });

  it("treats a missing connection list as empty", () => {
    expect(getCalendarConnectionKey(undefined)).toBe("");
  });
});

describe("useSyncWhenCalendarConnectionsChange", () => {
  it("syncs when a calendar connection appears after the first snapshot", () => {
    const scheduleSync = vi.fn();
    const { rerender } = renderHook(
      ({ connectionKey }) =>
        useSyncWhenCalendarConnectionsChange(connectionKey, scheduleSync),
      { initialProps: { connectionKey: "" } },
    );

    expect(scheduleSync).not.toHaveBeenCalled();

    rerender({ connectionKey: "google-1" });

    expect(scheduleSync).toHaveBeenCalledOnce();
  });

  it("does not sync again for the same connection key", () => {
    const scheduleSync = vi.fn();
    const { rerender } = renderHook(
      ({ connectionKey }) =>
        useSyncWhenCalendarConnectionsChange(connectionKey, scheduleSync),
      { initialProps: { connectionKey: "google-1" } },
    );

    rerender({ connectionKey: "google-1" });

    expect(scheduleSync).not.toHaveBeenCalled();
  });

  it("syncs again when another calendar account is added", () => {
    const scheduleSync = vi.fn();
    const { rerender } = renderHook(
      ({ connectionKey }) =>
        useSyncWhenCalendarConnectionsChange(connectionKey, scheduleSync),
      { initialProps: { connectionKey: "google-1" } },
    );

    rerender({ connectionKey: "google-1,google-2" });

    expect(scheduleSync).toHaveBeenCalledOnce();
  });
});
