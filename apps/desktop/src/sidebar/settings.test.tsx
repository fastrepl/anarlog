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
  isPro: true,
  isUpgradingToPro: false,
  select: vi.fn(),
  transitionChatMode: vi.fn(),
  upgradeToPro: vi.fn(),
  updateSettingsTabState: vi.fn(),
  updateTemplatesTabState: vi.fn(),
  workspaces: [] as Array<{ workspaceId: string }> | undefined,
  workspacesLoading: false,
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
  CustomSidebarHeader: () => <div />,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({
    isPro: mocks.isPro,
    isUpgradingToPro: mocks.isUpgradingToPro,
    upgradeToPro: mocks.upgradeToPro,
  }),
}));

vi.mock("~/settings/team/mirror", () => ({
  useMyWorkspacesWithMirror: () => ({
    data: mocks.workspaces,
    isLoading: mocks.workspacesLoading,
    isPending: mocks.workspacesLoading,
  }),
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
    mocks.isPro = true;
    mocks.isUpgradingToPro = false;
    mocks.openNew.mockClear();
    mocks.select.mockClear();
    mocks.transitionChatMode.mockClear();
    mocks.upgradeToPro.mockClear();
    mocks.updateSettingsTabState.mockClear();
    mocks.updateTemplatesTabState.mockClear();
    mocks.workspaces = [];
    mocks.workspacesLoading = false;
  });

  it("renders every settings menu label", () => {
    render(<SettingsNav />);

    [
      "App",
      "General",
      "Appearance",
      "Account",
      "Teams",
      "Notifications",
      "Workspace",
      "Meetings",
      "Folders",
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
      "Privacy",
      "Permissions",
      "Developers",
    ].forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
  });

  it.each([
    ["Calendar", { type: "calendar" }],
    ["Contacts", { type: "contacts" }],
    ["Folders", { type: "folders" }],
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

  it("opens Privacy inside settings", () => {
    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Privacy" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "privacy" },
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

  it("keeps locked Pro features visible and opens them", () => {
    mocks.isPro = false;

    render(<SettingsNav />);

    expect(screen.getByText("Sync")).toBeTruthy();
    expect(screen.getByText("Imports")).toBeTruthy();
    expect(
      screen.getAllByLabelText("Requires Anarlog Pro").length,
    ).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: /Sync/ }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "sync" },
    );
    expect(mocks.upgradeToPro).not.toHaveBeenCalled();
  });

  it.each([
    ["Teams", { tab: "team" }],
    ["Dictionary", { tab: "dictionary" }],
    ["Sync", { tab: "sync" }],
  ] as const)("opens locked %s navigation", (label, state) => {
    mocks.isPro = false;

    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: new RegExp(label) }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      state,
    );
  });

  it("opens locked Automations from settings", () => {
    mocks.isPro = false;

    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: /Automations/ }));

    expect(mocks.openNew).toHaveBeenCalledWith({ type: "automations" });
  });

  it("shows Teams with the Pro lock on the free plan", () => {
    mocks.isPro = false;

    render(<SettingsNav />);

    expect(screen.getByRole("button", { name: /Teams/ })).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: /Teams/ })
        .querySelector("[aria-label='Requires Anarlog Pro']"),
    ).toBeTruthy();
  });

  it("opens Teams for free members of an existing workspace", () => {
    mocks.isPro = false;
    mocks.workspaces = [{ workspaceId: "ws-1" }];

    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Teams" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "team" },
    );
    expect(
      screen
        .getByRole("button", { name: "Teams" })
        .querySelector("[aria-label='Requires Anarlog Pro']"),
    ).toBeNull();
  });

  it("does not lock Teams while workspaces are still loading", () => {
    mocks.isPro = false;
    mocks.workspaces = undefined;
    mocks.workspacesLoading = true;

    render(<SettingsNav />);

    fireEvent.click(screen.getByRole("button", { name: "Teams" }));

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "team" },
    );
    expect(
      screen
        .getByRole("button", { name: "Teams" })
        .querySelector("[aria-label='Requires Anarlog Pro']"),
    ).toBeNull();
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

    [
      "Meetings",
      "Folders",
      "Calendar",
      "Contacts",
      "Templates",
      "Automations",
    ].forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
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
