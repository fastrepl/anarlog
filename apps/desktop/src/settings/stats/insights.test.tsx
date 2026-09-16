import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ActivityRecord } from "./queries";

const mocks = vi.hoisted(() => ({
  activity: {
    data: [] as ActivityRecord[],
    isLoading: false,
    error: null as Error | null,
  },
}));

vi.mock("./badge-collection", () => ({ BadgeCollection: () => null }));
vi.mock("./queries", () => ({ useActivity: () => mocks.activity }));
vi.mock("~/calendar/hooks", () => ({
  useNow: () => new Date("2026-09-05T12:00:00Z"),
  useTimezone: () => "UTC",
  useWeekStartsOn: () => 1,
}));

import { SettingsInsights } from "./index";

function conversations(dates: string[]) {
  return dates.map((date, index) => ({
    session_id: String(index),
    created_at: date,
    started_at_ms: Date.parse(date),
    duration_ms: 30 * 60_000,
  }));
}

describe("personal insights page", () => {
  afterEach(cleanup);
  beforeEach(() => {
    mocks.activity = { data: [], isLoading: false, error: null };
  });

  it("shows evidence for the busiest weekday and updates it with the date range", () => {
    mocks.activity.data = conversations([
      "2026-08-19T10:00:00Z",
      "2026-08-26T10:00:00Z",
      "2026-09-02T10:00:00Z",
      "2026-09-03T10:00:00Z",
      "2026-09-04T10:00:00Z",
    ]);
    render(<SettingsInsights />);
    expect(screen.getByText("Most conversations: Wednesday")).toBeTruthy();
    expect(screen.getByText(/3 of 5 conversations \(60%\)/)).toBeTruthy();
    expect(screen.getByText("30 min")).toBeTruthy();
    const chart = screen.getByRole("region", {
      name: "Conversations by weekday",
    });
    expect(within(chart).getAllByRole("listitem")).toHaveLength(7);
    expect(within(chart).getAllByRole("listitem")[0].textContent).toBe(
      "Monday0",
    );
    fireEvent.click(screen.getByRole("button", { name: "7 days" }));
    expect(screen.queryByText("Most conversations: Wednesday")).toBeNull();
    expect(screen.getByText("A little more history will help")).toBeTruthy();
    expect(screen.getByText("Conversations: 3")).toBeTruthy();
  });

  it("does not invent a busiest day when weekdays tie", () => {
    mocks.activity.data = conversations([
      "2026-08-19T10:00:00Z",
      "2026-08-26T10:00:00Z",
      "2026-09-02T10:00:00Z",
      "2026-08-20T10:00:00Z",
      "2026-08-27T10:00:00Z",
      "2026-09-03T10:00:00Z",
    ]);
    render(<SettingsInsights />);
    expect(screen.getByText("No single busiest day")).toBeTruthy();
    expect(screen.getByText(/tied at 3 conversations/)).toBeTruthy();
  });

  it("distinguishes empty history from loading and failed queries", () => {
    mocks.activity.isLoading = true;
    const { rerender } = render(<SettingsInsights />);
    expect(screen.getByRole("status").textContent).toContain(
      "Loading your insights",
    );
    expect(screen.queryByText("A little more history will help")).toBeNull();
    mocks.activity.isLoading = false;
    mocks.activity.error = new Error("Database unavailable");
    rerender(<SettingsInsights />);
    expect(screen.getByRole("alert").textContent).toContain(
      "Couldn't load your insights",
    );
    expect(screen.queryByText("A little more history will help")).toBeNull();
    mocks.activity.error = null;
    rerender(<SettingsInsights />);
    expect(screen.getByText("A little more history will help")).toBeTruthy();
    expect(
      screen.queryByRole("region", { name: "Conversations by weekday" }),
    ).toBeNull();
    expect(screen.queryByText("Typical conversation length")).toBeNull();
  });
});
