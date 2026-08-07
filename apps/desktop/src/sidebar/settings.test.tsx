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
  transitionChatMode: vi.fn(),
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

vi.mock("./custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ title }: { title: ReactNode }) => <div>{title}</div>,
}));

vi.mock("~/store/zustand/tabs", () => {
  const getState = () => ({
    currentTab: mocks.currentTab,
    tabs: mocks.tabs,
    openNew: mocks.openNew,
    select: mocks.select,
    transitionChatMode: mocks.transitionChatMode,
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
    mocks.transitionChatMode.mockClear();
    mocks.updateSettingsTabState.mockClear();
    mocks.updateTemplatesTabState.mockClear();
  });

  it("renders every settings menu label", () => {
    render(<SettingsNav />);

    [
      "App",
      "General",
      "Appearance",
      "Account",
      "Notifications",
      "Workspace",
      "Meetings",
      "Calendar",
      "Contacts",
      "Templates",
      "Automations",
      "AI",
      "Transcription",
      "Intelligence",
      "Dictionary",
      "Data",
      "Sync",
      "Imports",
      "Advanced",
      "Permissions",
      "Developers",
    ].forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
  });

  it.each([
    ["Calendar", { type: "calendar" }],
    ["Contacts", { type: "contacts" }],
    ["Templates", { type: "templates" }],
    ["Automations", { type: "automations" }],
  ] as const)("opens the %s workspace", (label, destination) => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: label }));

    expect(
      screen.getByTestId(`settings-nav-destination-icon-${destination.type}`),
    ).toBeTruthy();
    expect(mocks.openNew).toHaveBeenCalledWith(destination);
  });

  it("opens runtime audio capabilities from the Permissions item", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Permissions" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      {
        tab: "permissions",
      },
    );
  });

  it("opens Appearance inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "appearance" },
    );
  });

  it("places dictionary in the AI section", () => {
    render(<SettingsNav />);

    expect(
      screen
        .getByText("Dictionary")
        .closest("button")
        ?.querySelector("[data-testid='settings-nav-icon-dictionary']"),
    ).toBeTruthy();
    expect(screen.queryByText("Personalization")).toBeNull();
  });

  it("opens Meetings inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Meetings" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "meetings" },
    );
  });

  it("opens Transcription inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Transcription" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "transcription" },
    );
  });

  it("opens Dictionary inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Dictionary" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "dictionary" },
    );
  });

  it("opens Sync inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Sync" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "sync" },
    );
  });

  it("opens Imports inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Imports" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "imports" },
    );
  });

  it("filters nav items by search query", () => {
    render(<SettingsNav />);

    fireEvent.change(screen.getByPlaceholderText("Search settings..."), {
      target: { value: "appear" },
    });

    expect(screen.getByText("Appearance")).toBeTruthy();
    expect(screen.queryByText("Meetings")).toBeNull();
    expect(screen.queryByText("Developers")).toBeNull();
  });

  it("keeps a whole group visible when its label matches", () => {
    render(<SettingsNav />);

    fireEvent.change(screen.getByPlaceholderText("Search settings..."), {
      target: { value: "workspace" },
    });

    ["Meetings", "Calendar", "Contacts", "Templates", "Automations"].forEach(
      (label) => {
        expect(screen.getByText(label)).toBeTruthy();
      },
    );
    expect(screen.queryByText("Appearance")).toBeNull();
  });

  it("shows an empty state when no settings match", () => {
    render(<SettingsNav />);

    fireEvent.change(screen.getByPlaceholderText("Search settings..."), {
      target: { value: "zzzzzz" },
    });

    expect(screen.getByText("No results found.")).toBeTruthy();
  });

  it("restores the full list when search is cleared", () => {
    render(<SettingsNav />);

    const input = screen.getByPlaceholderText("Search settings...");
    fireEvent.change(input, { target: { value: "audio" } });
    expect(screen.queryByText("Appearance")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));

    expect(screen.getByText("Appearance")).toBeTruthy();
  });

  it("clears the search on Escape", () => {
    render(<SettingsNav />);

    const input = screen.getByPlaceholderText("Search settings...");
    fireEvent.change(input, { target: { value: "audio" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect(screen.getByText("Appearance")).toBeTruthy();
  });
});
