import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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

  it("persists the main-language summary option without removing Korean transcription", async () => {
    mocks.useStoredSettingValuesQuery.mockReturnValue({
      data: {
        values: { ai_language: "en", spoken_languages: '["ko"]' },
        hasValues: new Set(["ai_language", "spoken_languages"]),
      },
      isLoading: false,
      error: null,
    });

    render(<SettingsApp />);
    fireEvent.click(
      screen.getByRole("switch", { name: "Use main language for summaries" }),
    );

    await waitFor(() => {
      expect(mocks.setSettingValues).toHaveBeenCalledWith(
        expect.objectContaining({
          summary_use_main_language: true,
          ai_language: "en",
          spoken_languages: '["ko"]',
        }),
      );
    });
  });

  it("keeps a language switched elsewhere when another setting changes", async () => {
    const storedLanguages = (ai_language: string, spoken: string[]) => ({
      data: {
        values: { ai_language, spoken_languages: JSON.stringify(spoken) },
        hasValues: new Set(["ai_language", "spoken_languages"]),
      },
      isLoading: false,
      error: null,
    });
    const toggleMeetingSetting = (value: boolean) => {
      const props = mocks.meetingSettingsProps.mock.lastCall?.[0] as {
        autoStopMeetings: { onChange: (value: boolean) => void };
      };
      act(() => props.autoStopMeetings.onChange(value));
    };
    mocks.useStoredSettingValuesQuery.mockReturnValue(
      storedLanguages("es", ["en"]),
    );

    const { rerender } = render(<SettingsMeetings />);
    toggleMeetingSetting(false);
    mocks.useStoredSettingValuesQuery.mockReturnValue(
      storedLanguages("en", ["es"]),
    );
    rerender(<SettingsMeetings />);
    toggleMeetingSetting(true);

    await waitFor(() => {
      expect(mocks.setSettingValues).toHaveBeenLastCalledWith(
        expect.objectContaining({
          auto_stop_meetings: true,
          ai_language: "en",
          spoken_languages: '["es"]',
        }),
      );
    });
  });

  it("keeps a newer edit when the write of an earlier one arrives late", async () => {
    const stored = (values: Record<string, unknown>) => ({
      data: { values, hasValues: new Set(Object.keys(values)) },
      isLoading: false,
      error: null,
    });
    const meetingProps = () =>
      mocks.meetingSettingsProps.mock.lastCall?.[0] as {
        autoStopMeetings: { onChange: (value: boolean) => void };
        autoStartScheduledMeetings: { onChange: (value: boolean) => void };
      };
    mocks.useStoredSettingValuesQuery.mockReturnValue(
      stored({ auto_stop_meetings: true }),
    );

    const { rerender } = render(<SettingsMeetings />);
    act(() => meetingProps().autoStopMeetings.onChange(false));
    await waitFor(() =>
      expect(mocks.setSettingValues).toHaveBeenCalledTimes(1),
    );
    act(() => meetingProps().autoStopMeetings.onChange(true));
    await waitFor(() =>
      expect(mocks.setSettingValues).toHaveBeenCalledTimes(2),
    );
    mocks.useStoredSettingValuesQuery.mockReturnValue(
      stored({ auto_stop_meetings: false }),
    );
    rerender(<SettingsMeetings />);
    act(() => meetingProps().autoStartScheduledMeetings.onChange(false));

    await waitFor(() => {
      expect(mocks.setSettingValues).toHaveBeenLastCalledWith(
        expect.objectContaining({
          auto_start_scheduled_meetings: false,
          auto_stop_meetings: true,
        }),
      );
    });
  });
});
