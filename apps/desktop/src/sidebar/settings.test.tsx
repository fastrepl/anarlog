import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: { type: "settings", state: { tab: "app" } } as {
    type: "settings";
    state: { tab?: string };
  } | null,
  tabs: [] as Array<{
    active: boolean;
    pinned: boolean;
    slotId: string;
    type: "templates";
    state: {
      showHomepage: boolean;
      isWebMode: boolean;
      selectedMineId: string | null;
      selectedWebIndex: number | null;
    };
  }>,
  openNew: vi.fn(),
  select: vi.fn(),
  updateSettingsTabState: vi.fn(),
  updateTemplatesTabState: vi.fn(),
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

vi.mock("~/store/zustand/tabs", () => {
  const getState = () => ({
    currentTab: mocks.currentTab,
    tabs: mocks.tabs,
    openNew: mocks.openNew,
    select: mocks.select,
    updateSettingsTabState: mocks.updateSettingsTabState,
    updateTemplatesTabState: mocks.updateTemplatesTabState,
  });
  const useTabs = Object.assign(
    (selector: (state: unknown) => unknown) => selector(getState()),
    { getState },
  );

  return {
    useTabs,
  };
});

import { SettingsNav } from "./settings";

describe("SettingsNav", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.currentTab = { type: "settings", state: { tab: "app" } };
    mocks.tabs = [];
    mocks.openNew.mockClear();
    mocks.select.mockClear();
    mocks.updateSettingsTabState.mockClear();
    mocks.updateTemplatesTabState.mockClear();
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
      "AI",
      "Transcription",
      "Intelligence",
      "Dictionary",
      "Templates",
    ].forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
  });

  it("places dictionary and templates in the AI section", () => {
    render(<SettingsNav />);

    expect(
      screen
        .getByText("Dictionary")
        .closest("button")
        ?.querySelector(".lucide-book-open"),
    ).toBeTruthy();
    expect(screen.queryByText("Personalization")).toBeNull();
  });

  it("opens Templates with Auto selected", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Templates" }));

    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "templates",
      state: {
        showHomepage: false,
        isWebMode: false,
        selectedMineId: "__auto__",
        selectedWebIndex: null,
      },
    });
  });

  it("selects Auto when reusing the Templates tab", () => {
    const templatesTab = {
      active: false,
      pinned: false,
      slotId: "templates-slot",
      type: "templates" as const,
      state: {
        showHomepage: false,
        isWebMode: false,
        selectedMineId: "template-1",
        selectedWebIndex: null,
      },
    };
    mocks.tabs = [templatesTab];
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Templates" }));

    expect(mocks.updateTemplatesTabState).toHaveBeenCalledWith(templatesTab, {
      showHomepage: false,
      isWebMode: false,
      selectedMineId: "__auto__",
      selectedWebIndex: null,
    });
    expect(mocks.select).toHaveBeenCalledWith(templatesTab);
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  it("opens Dictionary inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Dictionary" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "dictionary" },
    );
  });
});
