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

vi.mock("./queries", () => ({ useActivity: () => mocks.activity }));
vi.mock("./badge-collection", () => ({
  BadgeCollection: ({ records }: { records: ActivityRecord[] }) => (
    <section aria-label="Your badges">
      Lifetime records: {records.length}
    </section>
  ),
}));
vi.mock("~/calendar/hooks", () => ({
  useNow: () => new Date("2026-09-05T12:00:00Z"),
  useTimezone: () => "UTC",
  useWeekStartsOn: () => 1,
}));

import { SettingsInsights } from "./index";

describe("merged insights page", () => {
  afterEach(cleanup);
  beforeEach(() => {
    mocks.activity = { data: [], isLoading: false, error: null };
  });

  it("waits for activity before showing the badge collection", () => {
    mocks.activity.isLoading = true;
    const { rerender } = render(<SettingsInsights />);
    expect(screen.getByRole("status").textContent).toContain("Loading");
    expect(screen.queryByRole("progressbar")).toBeNull();
    mocks.activity.isLoading = false;
    rerender(<SettingsInsights />);
    expect(screen.getByRole("region", { name: "Your badges" })).toBeTruthy();
    expect(
      screen
        .getByRole("group", { name: "Date range" })
        .getAttribute("data-slot"),
    ).toBe("smooth-corners");
  });

  it("filters totals without filtering badge history or the yearly heatmap", async () => {
    mocks.activity.data = ["2026-08-01T12:00:00Z", "2026-09-04T12:00:00Z"].map(
      (date, index) => ({
        session_id: String(index),
        created_at: date,
        started_at_ms: Date.parse(date),
        duration_ms: 3_600_000,
      }),
    );
    render(<SettingsInsights />);
    const overview = screen.getByRole("region", { name: "Overview" });
    expect(
      within(overview).getByText("Conversations").nextElementSibling
        ?.textContent,
    ).toBe("2");
    expect(screen.getByText("Conversations: 2")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "7 days" }));
    expect(
      within(overview).getByText("Conversations").nextElementSibling
        ?.textContent,
    ).toBe("1");
    expect(screen.getByText("Conversations: 1")).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: "7 days" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(screen.getByText("Lifetime records: 2")).toBeTruthy();
    const day = screen.getByRole("listitem", {
      name: "August 1, 2026. Conversations: 1",
    });
    fireEvent.click(day);
    expect(
      await screen.findByText("August 1, 2026. Conversations: 1"),
    ).toBeTruthy();
    expect(
      screen
        .getByText("August 1, 2026. Conversations: 1")
        .getAttribute("data-slot"),
    ).toBe("smooth-corners");
  });

  it("reports query errors without showing misleading totals", () => {
    mocks.activity.error = new Error("Database unavailable");
    render(<SettingsInsights />);
    expect(screen.getByRole("alert").textContent).toContain("Couldn't load");
    expect(screen.queryByText("Conversations")).toBeNull();
  });
});
