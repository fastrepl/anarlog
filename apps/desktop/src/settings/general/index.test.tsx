import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  useStoredSettingValuesQuery: vi.fn(),
  mutateCloudSync: vi.fn(),
  setSettingValues: vi.fn(),
  meetingSettingsProps: vi.fn(),
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: { event: vi.fn() },
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: () => ({
    data: { configured: true },
    isLoading: false,
  }),
  useMutation: () => ({
    isPending: false,
    variables: undefined,
    mutate: mocks.mutateCloudSync,
  }),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: null, signOut: vi.fn() }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPro: true }),
}));

vi.mock("~/auth/cloudsync", () => ({
  applyCloudsyncPreference: vi.fn(),
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: vi.fn(),
  useSetSettingValues: () => mocks.setSettingValues,
  useStoredSettingValuesQuery: mocks.useStoredSettingValuesQuery,
}));

vi.mock("./account", () => ({ SettingsAccount: () => null }));
vi.mock("./app-settings", () => ({ AppSettingsView: () => null }));
vi.mock("./audio-settings", () => ({
  AudioSettingsView: () => <span>Audio settings</span>,
}));
vi.mock("./main-language", () => ({
  MainLanguageView: ({ value }: { value: string }) => (
    <span data-testid="main-language">{value}</span>
  ),
}));
vi.mock("./meeting-settings", () => ({
  MeetingSettingsView: (props: unknown) => {
    mocks.meetingSettingsProps(props);
    return <span>Meeting settings</span>;
  },
}));
vi.mock("./notification", () => ({ NotificationSettingsView: () => null }));
vi.mock("./permissions", () => ({ Permissions: () => null }));
vi.mock("./spoken-languages", () => ({ SpokenLanguagesView: () => null }));
vi.mock("./storage", () => ({ StorageSettingsView: () => null }));
vi.mock("./summary-length", () => ({
  SummaryLengthSelector: () => <span>Summary length selector</span>,
}));
vi.mock("./timezone", () => ({ TimezoneSelector: () => null }));
vi.mock("./week-start", () => ({ WeekStartSelector: () => null }));

import { SettingsApp, SettingsMeetings } from "./index";

describe("SettingsApp", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("waits for SQLite settings before constructing the form", () => {
    mocks.useStoredSettingValuesQuery.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    });

    render(<SettingsApp />);

    expect(screen.getByLabelText("Loading settings")).toBeTruthy();
  });

  it("constructs the form from the hydrated SQLite values", () => {
    mocks.useStoredSettingValuesQuery.mockReturnValue({
      data: {
        values: {
          ai_language: "ko",
          spoken_languages: JSON.stringify(["en"]),
        },
        hasValues: new Set(["ai_language", "spoken_languages"]),
      },
      isLoading: false,
      error: null,
    });

    render(<SettingsApp />);

    expect(screen.getByTestId("main-language").textContent).toBe("ko");
  });

  it("persists meeting switch toggles", async () => {
    mocks.useStoredSettingValuesQuery.mockReturnValue({
      data: {
        values: {},
        hasValues: new Set(),
      },
      isLoading: false,
      error: null,
    });

    render(<SettingsMeetings />);

    const props = mocks.meetingSettingsProps.mock.lastCall?.[0] as {
      autoStartScheduledMeetings: {
        value: boolean;
        onChange: (value: boolean) => void;
      };
    };
    expect(props.autoStartScheduledMeetings.value).toBe(true);

    act(() => props.autoStartScheduledMeetings.onChange(false));

    await waitFor(() => {
      expect(mocks.setSettingValues).toHaveBeenCalledWith(
        expect.objectContaining({ auto_start_scheduled_meetings: false }),
      );
    });
  });

  it("keeps audio controls with meeting settings", () => {
    mocks.useStoredSettingValuesQuery.mockReturnValue({
      data: {
        values: {},
        hasValues: new Set(),
      },
      isLoading: false,
      error: null,
    });

    render(<SettingsMeetings />);

    expect(screen.getByText("Meetings")).toBeTruthy();
    expect(screen.getByText("Meeting settings")).toBeTruthy();
    expect(screen.getByText("Summaries")).toBeTruthy();
    expect(screen.getByText("Summary length selector")).toBeTruthy();
    expect(screen.getByText("Audio")).toBeTruthy();
    expect(screen.getByText("Audio settings")).toBeTruthy();
  });
});
