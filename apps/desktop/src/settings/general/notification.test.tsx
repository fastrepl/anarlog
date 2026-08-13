import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  clearNotifications: vi.fn(),
  currentPlatform: "macos",
  setSettingValues: vi.fn(),
  useConfigValues: vi.fn(),
  useQuery: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (input: TemplateStringsArray | string) =>
      typeof input === "string" ? input : input.join(""),
  }),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: mocks.useQuery,
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.currentPlatform,
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: {
    listDefaultIgnoredBundleIds: vi.fn(),
    listInstalledApplications: vi.fn(),
  },
}));

vi.mock("@anlg/plugin-notification", () => ({
  commands: {
    clearNotifications: mocks.clearNotifications,
  },
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValues: () => mocks.setSettingValues,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: mocks.useConfigValues,
}));

import { NotificationSettingsView } from "./notification";

const baseConfig = {
  notification_event: false,
  notification_detect: true,
  notification_bounce: true,
  show_app_in_dock: true,
  respect_dnd: false,
  ignored_platforms: [],
  included_platforms: [],
  mic_active_threshold: 15,
};

describe("NotificationSettingsView", () => {
  beforeEach(() => {
    mocks.clearNotifications.mockReset();
    mocks.currentPlatform = "macos";
    mocks.setSettingValues.mockReset();
    mocks.useConfigValues.mockReset();
    mocks.useConfigValues.mockReturnValue(baseConfig);
    mocks.useQuery.mockImplementation(
      ({ queryKey }: { queryKey: readonly string[] }) => {
        if (queryKey[1] === "all-installed-applications") {
          return {
            data: [{ id: "com.ting.aqua-bridge", name: "Ting Aqua Bridge" }],
          };
        }
        return { data: [] };
      },
    );
  });

  afterEach(cleanup);

  it("shows persisted ignored apps as soon as the form hydrates", async () => {
    const { rerender } = render(<NotificationSettingsView />);

    expect(screen.queryByText("Ting Aqua Bridge")).toBeNull();

    mocks.useConfigValues.mockReturnValue({
      ...baseConfig,
      ignored_platforms: ["com.ting.aqua-bridge"],
    });
    rerender(<NotificationSettingsView />);

    await waitFor(() =>
      expect(screen.getByText("Ting Aqua Bridge")).toBeTruthy(),
    );
  });

  it("hides unsupported microphone detection and DND controls on Windows", () => {
    mocks.currentPlatform = "windows";

    render(<NotificationSettingsView />);

    expect(screen.getByText("Event notifications")).toBeTruthy();
    expect(screen.queryByText("Microphone detection")).toBeNull();
    expect(screen.queryByText("Respect Do-Not-Disturb mode")).toBeNull();
  });

  it("shows one general app icon bounce control", () => {
    render(<NotificationSettingsView />);

    expect(screen.getByText("Bounce app icon")).toBeTruthy();
    expect(
      screen.getByText(
        "Get your attention when Anarlog finishes work in the background.",
      ),
    ).toBeTruthy();
    expect(
      screen.queryByText("Bounce app icon when a transcript is ready"),
    ).toBeNull();
    expect(
      screen.queryByText("Bounce app icon when a summary is ready"),
    ).toBeNull();
  });

  it("hides app icon bouncing when the app is hidden from the Dock", () => {
    mocks.useConfigValues.mockReturnValue({
      ...baseConfig,
      show_app_in_dock: false,
    });

    render(<NotificationSettingsView />);

    expect(screen.queryByText("Bounce app icon")).toBeNull();
  });
});
