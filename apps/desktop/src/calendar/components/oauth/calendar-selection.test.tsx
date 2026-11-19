import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  scheduleSync: vi.fn(),
  calendars: [] as Array<{
    id: string;
    name: string;
    enabled: boolean;
    source: string;
    color: string;
    connection_id: string;
  }>,
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({
    invalidateQueries: vi.fn(),
  }),
}));

vi.mock("../context", () => ({
  useSync: () => ({
    cancelDebouncedSync: vi.fn(),
    status: "idle",
    scheduleDebouncedSync: vi.fn(),
    scheduleSync: mocks.scheduleSync,
  }),
}));

vi.mock("~/calendar/queries", () => ({
  setCalendarEnabled: vi.fn(),
  useCalendarRows: () => mocks.calendars,
}));

import { PROVIDERS } from "../shared";
import {
  OAuthCalendarSelection,
  useOAuthCalendarSelection,
} from "./calendar-selection";

import { setCalendarSyncIssue } from "~/services/calendar/sync-errors";

const GOOGLE_PROVIDER = PROVIDERS.find((provider) => provider.id === "google")!;

function HookHarness() {
  useOAuthCalendarSelection(GOOGLE_PROVIDER, []);
  return null;
}

describe("useOAuthCalendarSelection", () => {
  afterEach(() => {
    cleanup();
    mocks.scheduleSync.mockClear();
    mocks.calendars = [];
  });

  it("shows the mailbox error on the affected account before calendar discovery and clears it after retry", () => {
    const provider = PROVIDERS.find((item) => item.id === "outlook")!;
    const connections = [
      {
        integration_id: "outlook",
        connection_id: "fixture",
        account_identity: "Work account",
      },
    ];
    function OutlookSelection() {
      const { groups, handleToggle, handleRefresh, isLoading } =
        useOAuthCalendarSelection(provider, connections);
      return (
        <OAuthCalendarSelection
          groups={groups}
          onToggle={handleToggle}
          onRefresh={handleRefresh}
          isLoading={isLoading}
        />
      );
    }
    setCalendarSyncIssue("outlook:fixture", "outlook_mailbox_unavailable");
    render(<OutlookSelection />);
    expect(screen.getByText("Work account")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("Exchange Online");
    expect(screen.queryByText("No calendars found")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Refresh calendars" }));
    expect(mocks.scheduleSync).toHaveBeenCalled();
    act(() => setCalendarSyncIssue("outlook:fixture"));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("No calendars found")).toBeTruthy();
  });

  it.each([
    ["still empty", []],
    [
      "already present",
      [
        {
          id: "cal-1",
          name: "Work",
          enabled: true,
          source: "user@example.com",
          color: "#4285f4",
          connection_id: "conn-1",
        },
      ],
    ],
  ])("does not sync on mount when calendar rows are %s", (_label, rows) => {
    mocks.calendars = rows;

    render(<HookHarness />);

    expect(mocks.scheduleSync).not.toHaveBeenCalled();
  });
});
