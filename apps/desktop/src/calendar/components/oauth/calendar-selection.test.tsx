import { cleanup, render } from "@testing-library/react";
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
import { useOAuthCalendarSelection } from "./calendar-selection";

const GOOGLE_PROVIDER = PROVIDERS.find((provider) => provider.id === "google")!;

function HookHarness() {
  useOAuthCalendarSelection(GOOGLE_PROVIDER);
  return null;
}

describe("useOAuthCalendarSelection", () => {
  afterEach(() => {
    cleanup();
    mocks.scheduleSync.mockClear();
    mocks.calendars = [];
  });

  it("syncs when a newly connected account has no calendars yet", () => {
    render(<HookHarness />);

    expect(mocks.scheduleSync).toHaveBeenCalledOnce();
  });

  it("does not resync an account that already has calendars", () => {
    mocks.calendars = [
      {
        id: "cal-1",
        name: "Work",
        enabled: true,
        source: "user@example.com",
        color: "#4285f4",
        connection_id: "conn-1",
      },
    ];

    render(<HookHarness />);

    expect(mocks.scheduleSync).not.toHaveBeenCalled();
  });
});
