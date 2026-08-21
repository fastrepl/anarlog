import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PastSessionNote } from "~/session/insights/past-notes";

const mocks = vi.hoisted(() => ({
  event: {
    title: "Weekly Product Sync",
    started_at: "2026-08-21T09:00:00.000Z",
    ended_at: "2026-08-21T10:00:00.000Z",
    is_all_day: false,
    location: "Studio",
    description: "Review the launch plan and open questions.",
  } as {
    title: string;
    started_at: string;
    ended_at: string;
    is_all_day: boolean;
    location?: string;
    description?: string;
  } | null,
  now: new Date("2026-08-21T08:00:00.000Z"),
  notes: [] as PastSessionNote[],
  participants: [
    { name: "Ada", email: "ada@example.com" },
    { name: "Current User", is_current_user: true },
  ],
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({ i18n: { locale: "en-US" } }),
}));

vi.mock("~/calendar/hooks", () => ({
  useNow: () => mocks.now,
}));

vi.mock("~/calendar/queries", () => ({
  useSessionCalendarEvent: () =>
    mocks.event ? { ...mocks.event, participants: mocks.participants } : null,
}));

vi.mock("~/session/insights/past-notes", () => ({
  usePastSessionNotes: () => ({
    notes: mocks.notes,
    hasPastNotes: mocks.notes.length > 0,
    isGenerating: false,
    canGenerate: false,
    regenerate: vi.fn(),
    regenerateAll: vi.fn(),
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => "UTC",
}));

import {
  PreMeetingBrief,
  shouldShowPreMeetingBrief,
} from "./pre-meeting-brief";

describe("PreMeetingBrief", () => {
  beforeEach(() => {
    mocks.event = {
      title: "Weekly Product Sync",
      started_at: "2026-08-21T09:00:00.000Z",
      ended_at: "2026-08-21T10:00:00.000Z",
      is_all_day: false,
      location: "Studio",
      description: "Review the launch plan and open questions.",
    };
    mocks.now = new Date("2026-08-21T08:00:00.000Z");
    mocks.notes = [];
    mocks.participants = [
      { name: "Ada", email: "ada@example.com" },
      { name: "Current User", is_current_user: true },
    ];
  });

  afterEach(cleanup);

  it("shows calendar details and facts from the latest related meeting", () => {
    mocks.notes = [
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "Aug 14, 2026",
        occurredAt: "2026-08-14T09:00:00.000Z",
        participantNames: ["Ada"],
        sourceSummary: "A longer raw summary.",
        relationship: "same_series",
        summary:
          "- Ada will share the prototype.\n- Confirm launch timing with Sam.",
        isGenerating: false,
      },
    ];

    render(<PreMeetingBrief sessionId="current" />);

    expect(screen.getByText("Pre-meeting brief")).toBeTruthy();
    expect(screen.getByText("Studio")).toBeTruthy();
    expect(screen.getByText("Ada")).toBeTruthy();
    expect(
      screen.getByText("Review the launch plan and open questions."),
    ).toBeTruthy();
    expect(screen.getByText("Last meeting · Aug 14, 2026")).toBeTruthy();
    expect(screen.getByText("Ada will share the prototype.")).toBeTruthy();
    expect(screen.getByText("Confirm launch timing with Sam.")).toBeTruthy();
    expect(screen.queryByText("Current User")).toBeNull();
  });

  it("uses source summary text while generated facts are unavailable", () => {
    mocks.notes = [
      {
        sessionId: "previous",
        title: "Weekly Product Sync",
        dateLabel: "Aug 14, 2026",
        occurredAt: "2026-08-14T09:00:00.000Z",
        sourceSummary: "Decided to ship the smaller onboarding experiment.",
        relationship: "same_series",
        summary: null,
        isGenerating: false,
      },
    ];

    render(<PreMeetingBrief sessionId="current" />);

    expect(
      screen.getByText("Decided to ship the smaller onboarding experiment."),
    ).toBeTruthy();
  });

  it("does not present title-only matches as prior meeting history", () => {
    mocks.notes = [
      {
        sessionId: "possible-match",
        title: "Weekly Product Sync",
        dateLabel: "Aug 14, 2026",
        occurredAt: "2026-08-14T09:00:00.000Z",
        sourceSummary: "Context from a different recurring meeting.",
        relationship: "matching_title",
        summary: null,
        isGenerating: false,
      },
    ];

    render(<PreMeetingBrief sessionId="current" />);

    expect(
      screen.queryByText("Context from a different recurring meeting."),
    ).toBeNull();
    expect(
      screen.getByText(/No previous meeting summary is available/),
    ).toBeTruthy();
  });

  it("handles meetings without prior summaries", () => {
    render(<PreMeetingBrief sessionId="current" />);

    expect(
      screen.getByText(/No previous meeting summary is available/),
    ).toBeTruthy();
  });

  it("stays hidden for past and all-day events", () => {
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-21T07:00:00.000Z",
          ended_at: "2026-08-21T07:30:00.000Z",
          is_all_day: false,
        },
        mocks.now.getTime(),
      ),
    ).toBe(false);
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-21T07:58:00.000Z",
          ended_at: "",
          is_all_day: false,
        },
        mocks.now.getTime(),
      ),
    ).toBe(true);
    expect(
      shouldShowPreMeetingBrief(
        {
          started_at: "2026-08-22T07:00:00.000Z",
          ended_at: "2026-08-22T07:30:00.000Z",
          is_all_day: true,
        },
        mocks.now.getTime(),
      ),
    ).toBe(false);

    mocks.event = {
      title: "Past meeting",
      started_at: "2026-08-21T07:00:00.000Z",
      ended_at: "2026-08-21T07:30:00.000Z",
      is_all_day: false,
    };
    render(<PreMeetingBrief sessionId="current" />);

    expect(screen.queryByText("Pre-meeting brief")).toBeNull();
  });

  it("stays hidden when the canonical calendar event was removed", () => {
    mocks.event = null;

    render(<PreMeetingBrief sessionId="current" />);

    expect(screen.queryByText("Pre-meeting brief")).toBeNull();
  });
});
