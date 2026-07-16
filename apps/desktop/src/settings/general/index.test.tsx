import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  useStoredSettingValuesQuery: vi.fn(),
  mutateCloudSync: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
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
  useSetSettingValues: vi.fn(),
  useStoredSettingValuesQuery: mocks.useStoredSettingValuesQuery,
}));

vi.mock("./account", () => ({ SettingsAccount: () => null }));
vi.mock("./app-settings", () => ({ AppSettingsView: () => null }));
vi.mock("./main-language", () => ({
  MainLanguageView: ({ value }: { value: string }) => (
    <span data-testid="main-language">{value}</span>
  ),
}));
vi.mock("./notification", () => ({ NotificationSettingsView: () => null }));
vi.mock("./permissions", () => ({ Permissions: () => null }));
vi.mock("./spoken-languages", () => ({ SpokenLanguagesView: () => null }));
vi.mock("./storage", () => ({ StorageSettingsView: () => null }));
vi.mock("./theme", () => ({ ThemeSelector: () => null }));
vi.mock("./timezone", () => ({ TimezoneSelector: () => null }));
vi.mock("./week-start", () => ({ WeekStartSelector: () => null }));

import { SettingsApp } from "./index";

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
});
