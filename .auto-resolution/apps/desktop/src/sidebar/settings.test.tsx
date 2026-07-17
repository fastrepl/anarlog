import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "settings", state: { tab: "app" } } as {
    type: "settings";
    state: { tab?: string };
  } | null,
  openNew: vi.fn(),
  updateSettingsTabState: vi.fn(),
}));

const lingui = vi.hoisted(() => {
  const t = (
    input: TemplateStringsArray | { message?: string } | string,
    ...values: unknown[]
  ) => {
    if (Array.isArray(input)) {
      return input.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      );
    }

    if (typeof input === "string") {
      return input;
    }

    if ("message" in input) {
      return input.message ?? "";
    }

    return "";
  };

  return { t };
});

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
  }: {
    children?: ReactNode;
    id?: string;
    message?: string;
  }) => <>{children ?? message ?? id}</>,
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("./custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ title }: { title: ReactNode }) => <div>{title}</div>,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: unknown) => unknown) =>
    selector({
      currentTab: mocks.currentTab,
      openNew: mocks.openNew,
      updateSettingsTabState: mocks.updateSettingsTabState,
    }),
}));

import { SettingsNav } from "./settings";

describe("SettingsNav", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.currentTab = { type: "settings", state: { tab: "app" } };
    mocks.openNew.mockClear();
    mocks.updateSettingsTabState.mockClear();
  });

  it("renders every settings menu label", () => {
    render(<SettingsNav />);

    [
      "General",
      "App",
      "Account",
      "Notifications",
      "Developers",
      "Permissions",
      "Context",
      "Calendar",
      "Contacts",
      "Templates",
      "AI",
      "Transcription",
      "Intelligence",
      "Personalization",
    ].forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
  });

  it("uses a smile icon for personalization", () => {
    render(<SettingsNav />);

    const button = screen.getByText("Personalization").closest("button");
    expect(button?.querySelector(".lucide-smile")).toBeTruthy();
  });
});
